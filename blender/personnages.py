# Personnages animés -> fichiers .vat (animation de sommets), repris de P-24
# à P-26.
# Modèles CC0 de Quaternius ; teintes refaites (peau grise et sang des
# zombies, treillis des soldats), bras tendus des zombies ajoutés à la pose.
# Repère de sortie : +X avant, +Z haut. Format (petit-boutien) :
#   "P24V", u32 nb sommets, u32 nb indices, u32 nb images, u32 nb animations,
#   animations (u32 première image, u32 nb images, f32 images/s),
#   couleurs f32x4 (linéaires) par sommet, indices u32,
#   images : f32x3 position + f32 normale (octaèdre, 2 x 12 bits) par sommet.
# Sources : <racine>/<id>/<nom>, rangées par `assets telecharger`.
# blender -b --factory-startup -P blender/personnages.py -- <racine> <sortie>
import bpy, sys, math, mathutils, os, struct, random
RACINE, SORTIE = sys.argv[sys.argv.index('--') + 1:][:2]
os.makedirs(SORTIE, exist_ok=True)
# Modèle Quaternius -> dossier du manifeste.
ID = {'casual': 'quaternius-casual', 'worker': 'quaternius-worker', 'farmer': 'quaternius-farmer',
      'adv': 'quaternius-adventurer', 'swat': 'quaternius-swat', 'rifle': 'quaternius-assault-rifle'}
IPS = 15

def lin(h):
    c = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    return [x ** 2.2 for x in c]

# Teintes par nom de matériau.
PEAU = {'Skin': '8b957a', 'Skin_Darker': '6f785f', 'Eye': 'd8d4a0', 'Eyebrows': '2a2620', 'Hair': '2e2821', 'Moustache': '2e2821'}
Z = {
    'casual': {**PEAU, 'LightBrown': '8d8a80', 'LightBlue': '3b4656', 'White': '9a9488', 'Red_Dark': '4a2a26'},
    'worker': {**PEAU, 'Skin': '8f9a8a', 'Worker_Yellow': '8a7636', 'Worker_Vest': '80502c', 'LightBrown': '6d6a58', 'Black': '23211f', 'Grey': '4b4a48', 'Brown2': '4d4333', 'Brown': '3f3629'},
    'farmer': {**PEAU, 'Skin': '97936f', 'Brown': '54432f', 'Brown2': '3d3226', 'LightBlue': '364660', 'Beige': '8f8062', 'Red': '6b2a24'},
    'adv': {**PEAU, 'Skin': '7f8a78', 'Green': '46523a', 'LightGreen': '5b6448', 'Black': '24221f', 'Grey': '4c4b48', 'Brown2': '57492f', 'Brown': '4a3d2a', 'Gold': '6b5a2c'},
}
SOLDAT = {'Swat': '5b6240', 'Swat_Black': '3b3f2a', 'Skin': 'b98a68', 'Visor': '14181a', 'Main': '1c1d1c', 'MainDark': '121312', 'MainLight': '2a2b29'}

def charger(f):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=os.path.join(RACINE, ID[f], f + '.glb'))
    for o in [o for o in bpy.data.objects if o.name.startswith('Icosphere')]:
        bpy.data.objects.remove(o)
    arm = next(o for o in bpy.data.objects if o.type == 'ARMATURE')
    return arm, [o for o in bpy.data.objects if o.type == 'MESH']

def action(nom):
    for a in bpy.data.actions:
        if a.name.split('|')[-1].split('_CharacterArmature')[0] == nom:
            return a
    raise KeyError(nom + ' ' + str([a.name for a in bpy.data.actions]))

def poser(arm, a):
    ad = arm.animation_data or arm.animation_data_create()
    for t in ad.nla_tracks: t.mute = True
    ad.action = a
    if hasattr(ad, 'action_suitable_slots') and len(ad.action_suitable_slots):
        ad.action_slot = ad.action_suitable_slots[0]

def tendre_bras(arm, t, ampl):
    """Bras tendus vers l'avant, buste penché : démarche de zombie."""
    bpy.context.view_layer.update()
    W = arm.matrix_world
    # Dans le monde de l'import : avant = -Y, haut = +Z.
    for nom, cote in (('UpperArm.L', 1), ('UpperArm.R', -1)):
        b = arm.pose.bones.get(nom)
        if b is None:
            continue
        d = (W.to_3x3() @ (b.tail - b.head)).normalized()
        cible = mathutils.Vector((cote * 0.22, -1, -0.12 + ampl * math.sin(t + cote))).normalized()
        q = d.rotation_difference(cible)
        m = W @ b.matrix
        p = m.to_translation()
        r = mathutils.Matrix.Translation(p) @ q.to_matrix().to_4x4() @ mathutils.Matrix.Translation(-p)
        b.matrix = W.inverted() @ r @ m
        bpy.context.view_layer.update()
        f = arm.pose.bones.get(nom.replace('Upper', 'Lower'))
        if f:
            d = (W.to_3x3() @ (f.tail - f.head)).normalized()
            cible = mathutils.Vector((-cote * 0.1, -1, 0.05)).normalized()
            q = d.rotation_difference(cible)
            m = W @ f.matrix
            p = m.to_translation()
            r = mathutils.Matrix.Translation(p) @ q.to_matrix().to_4x4() @ mathutils.Matrix.Translation(-p)
            f.matrix = W.inverted() @ r @ m
            bpy.context.view_layer.update()

def octa(n):
    s = abs(n.x) + abs(n.y) + abs(n.z) or 1
    x, y = n.x / s, n.y / s
    if n.z < 0:
        x, y = (1 - abs(y)) * (1 if x >= 0 else -1), (1 - abs(x)) * (1 if y >= 0 else -1)
    return float(int((x * .5 + .5) * 4095 + .5) * 4096 + int((y * .5 + .5) * 4095 + .5))

def cuire(f, sortie, teintes, anims, tris, zombie, arme=None):
    random.seed(sortie)
    arm, obs = charger(f)
    for o in obs:
        for m in [m for m in o.modifiers if m.type != 'ARMATURE']:
            o.modifiers.remove(m)
    bpy.ops.object.select_all(action='DESELECT')
    for o in obs: o.select_set(True)
    bpy.context.view_layer.objects.active = obs[0]
    if len(obs) > 1: bpy.ops.object.join()
    ob = bpy.context.view_layer.objects.active
    n = sum(len(p.vertices) - 2 for p in ob.data.polygons)
    # Fusion des sommets doublés par l'export, puis réduction.
    bpy.ops.object.mode_set(mode='EDIT'); bpy.ops.mesh.select_all(action='SELECT')
    bpy.ops.mesh.remove_doubles(threshold=max(ob.dimensions) / max(ob.matrix_world.to_scale()) * 1e-5); bpy.ops.object.mode_set(mode='OBJECT')
    n = sum(len(p.vertices) - 2 for p in ob.data.polygons)
    print('fusion', sortie, n)
    d = ob.modifiers.new('d', 'DECIMATE'); d.ratio = min(1, tris / n)
    while ob.modifiers[0] != d: bpy.ops.object.modifier_move_up(modifier='d')
    bpy.ops.object.modifier_apply(modifier='d')
    me = ob.data
    me.calc_loop_triangles()
    noms = [m.name.split('.')[0] for m in me.materials]
    # Sommets du jeu : (sommet, matériau).
    cle, sommets, idx = {}, [], []
    for t in me.loop_triangles:
        for v in t.vertices:
            k = (v, t.material_index)
            if k not in cle:
                cle[k] = len(sommets); sommets.append(k)
            idx.append(cle[k])
    sang = [mathutils.Vector((random.uniform(-.3, .3), random.uniform(-.4, .1), random.uniform(.5, 1.75))) for _ in range(9)] if zombie else []
    cols = []
    for v, mi in sommets:
        c = list(lin(teintes[noms[mi]]))
        p = ob.matrix_world @ me.vertices[v].co
        k = 0.82 + 0.36 * random.random()
        c = [x * k for x in c]
        if any((p - s).length < 0.1 for s in sang) and not noms[mi].startswith('Hair'):
            c = [0.13 * k, 0.012, 0.01]
        cols.append(c + [1.0])
    # Arme : maillage rigide suivant la main droite.
    av, ai, ac = [], [], []
    if arme:
        av0 = set(bpy.data.objects)
        bpy.ops.import_scene.gltf(filepath=os.path.join(RACINE, ID[arme], arme + '.glb'))
        g = [o for o in set(bpy.data.objects) - av0 if o.type == 'MESH'][0]
        bpy.context.view_layer.objects.active = g
        d = g.modifiers.new('d', 'DECIMATE'); d.ratio = min(1, tris * 0.3 / sum(len(p.vertices) - 2 for p in g.data.polygons))
        bpy.ops.object.select_all(action='DESELECT'); g.select_set(True); bpy.ops.object.modifier_apply(modifier='d')
        g.data.calc_loop_triangles()
        gn = [m.name.split('.')[0] for m in g.data.materials]
        ech = 0.9 / 5.17
        ck = {}
        for t in g.data.loop_triangles:
            for v in t.vertices:
                k = (v, t.material_index)
                if k not in ck:
                    ck[k] = len(av)
                    # Canon +X, haut +Z ; origine à la poignée.
                    q = g.matrix_world @ g.data.vertices[v].co
                    av.append((q - mathutils.Vector((-0.1, 0, -0.1))) * ech)
                    ac.append(list(lin(SOLDAT[gn[t.material_index]])) + [1.0])
                ai.append(ck[k])
        g.hide_set(True)
        idx += [i + len(sommets) for i in ai]
        cols += ac
    images, table = [], []
    sc = bpy.context.scene
    for nom, nimg, tend in anims:
        poser(arm, action(nom))
        f0, f1 = action(nom).frame_range
        table.append((len(images), nimg, nimg / ((f1 - f0) / sc.render.fps)))
        for k in range(nimg):
            x = f0 + (f1 - f0) * k / (nimg if tend is not None or nom not in ('Death',) else nimg - 1)
            sc.frame_set(int(x), subframe=x - int(x))
            if zombie and tend is not None:
                tendre_bras(arm, 2 * math.pi * k / nimg, tend)
            dg = bpy.context.evaluated_depsgraph_get()
            ev = ob.evaluated_get(dg)
            mm = ev.data
            W = ev.matrix_world
            N3 = W.to_3x3()
            img = []
            for v, _ in sommets:
                p = W @ mm.vertices[v].co
                nn = (N3 @ mm.vertices[v].normal).normalized()
                # avant -Y -> +X : (x, y) -> (-y, x)
                img += [-p.y, p.x, p.z, octa(mathutils.Vector((-nn.y, nn.x, nn.z)))]
            if arme:
                pa = arm.evaluated_get(dg)
                def os_(n):
                    b = pa.pose.bones[n]; return pa.matrix_world @ b.head, pa.matrix_world @ b.tail
                h, t2 = os_('Wrist.R') if 'Wrist.R' in pa.pose.bones else os_('LowerArm.R')
                h2, _ = os_('Wrist.L') if 'Wrist.L' in pa.pose.bones else os_('LowerArm.L')
                main = (h + t2) / 2
                viser = 'Gun' in nom or 'Shoot' in nom
                dirx = mathutils.Vector((0, -1, -0.03)) if viser else mathutils.Vector((0.75, -0.55, -0.35))
                dirx.normalize()
                up = mathutils.Vector((0, 0, 1)); cot = up.cross(dirx).normalized(); up = dirx.cross(cot)
                Mx = mathutils.Matrix((dirx, cot, up)).transposed()
                for q in av:
                    p = main + Mx @ q
                    nn = (Mx @ mathutils.Vector((0, 1 if q.y > 0 else -1, .4))).normalized()
                    img += [-p.y, p.x, p.z, octa(mathutils.Vector((-nn.y, nn.x, nn.z)))]
            images.append(img)
    nv = len(sommets) + len(av)
    with open(os.path.join(SORTIE, sortie + '.vat'), 'wb') as o:
        o.write(b'P24V' + struct.pack('<4I', nv, len(idx), len(images), len(table)))
        for t in table: o.write(struct.pack('<IIf', *t))
        for c in cols: o.write(struct.pack('<4f', *c))
        o.write(struct.pack('<%dI' % len(idx), *idx))
        for im in images: o.write(struct.pack('<%df' % len(im), *im))
    xs = [images[0][i::4] for i in range(3)]
    print('VAT', sortie, 'sommets', nv, 'triangles', len(idx) // 3, 'images', len(images), table, [(round(min(a), 2), round(max(a), 2)) for a in xs])

ZA = [('Idle', 10, 0.15), ('Walk', 20, 0.12), ('Run', 16, 0.25), ('Punch_Right', 12, None), ('Death', 14, None)]
SA = [('Idle_Gun', 10, None), ('Run', 16, None), ('Idle_Gun_Shoot', 12, None), ('Death', 14, None)]
for f, t in Z.items():
    cuire(f, 'zombie_' + f, t, ZA, 1100, True)
    cuire(f, 'zombie_' + f + '_loin', t, ZA, 320, True)
cuire('swat', 'soldat', SOLDAT, SA, 2200, False, 'rifle')
