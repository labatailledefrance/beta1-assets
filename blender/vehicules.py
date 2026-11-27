# Modèles de véhicules -> GLB à l'échelle du jeu (repris de P-24 à P-26).
# Repère de sortie : +X avant, +Z haut, origine au sol (sommets écrits tels
# quels, sans conversion Y haut). Une pièce par fichier (caisse, tourelle).
# Sources : <racine>/<id>/<nom>, rangées par `assets telecharger`.
# blender -b --factory-startup -P blender/vehicules.py -- <racine> <sortie>
import bpy, sys, math, mathutils, os
RACINE, SORTIE = sys.argv[sys.argv.index('--') + 1:][:2]
os.makedirs(SORTIE, exist_ok=True)

def vider():
    bpy.ops.wm.read_factory_settings(use_empty=True)

def importer(f):
    vider()
    bpy.ops.import_scene.gltf(filepath=os.path.join(RACINE, f))
    for o in [o for o in bpy.data.objects if o.name.startswith('Icosphere')]:
        bpy.data.objects.remove(o)
    obs = [o for o in bpy.data.objects if o.type == 'MESH']
    for o in obs:
        m = o.matrix_world.copy()
        o.parent = None
        o.matrix_world = m
    for o in list(bpy.data.objects):
        if o.type != 'MESH':
            bpy.data.objects.remove(o)
    bpy.ops.object.select_all(action='SELECT')
    bpy.context.view_layer.objects.active = obs[0]
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    return obs

def joindre(obs):
    bpy.ops.object.select_all(action='DESELECT')
    for o in obs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = obs[0]
    bpy.ops.object.join()
    return bpy.context.view_layer.objects.active

def mats(ob, mini=0.02):
    """Matériaux mats opaques : texture ou couleur de base, sans métal ni émission."""
    for i, s in enumerate(ob.material_slots):
        m = s.material
        tex = None; col = (0.5, 0.5, 0.5, 1)
        for n in m.node_tree.nodes:
            if n.type == 'TEX_IMAGE' and tex is None:
                tex = n.image
        for n in m.node_tree.nodes:
            if n.type == 'BSDF_PRINCIPLED':
                col = tuple(n.inputs['Base Color'].default_value)
            elif n.type in ('RGB',):
                col = tuple(n.outputs[0].default_value)
            elif n.type in ('EMISSION', 'BACKGROUND'):
                col = tuple(n.inputs['Color'].default_value)
            elif n.type == 'MIX' and tex is None:
                for k in n.inputs:
                    if k.type == 'RGBA' and not k.is_linked:
                        c = tuple(k.default_value)
                        if c[:3] not in ((0, 0, 0), (1, 1, 1)): col = c
        nm = bpy.data.materials.new('m_' + m.name)
        nm.use_nodes = True
        nt = nm.node_tree
        b = next(n for n in nt.nodes if n.type == 'BSDF_PRINCIPLED')
        b.inputs['Metallic'].default_value = 0
        b.inputs['Roughness'].default_value = 0.85
        if 'lass' in m.name:
            col = (0.03, 0.045, 0.05, 1); b.inputs['Roughness'].default_value = 0.2; tex = None
        if tex:
            t = nt.nodes.new('ShaderNodeTexImage'); t.image = tex
            nt.links.new(t.outputs['Color'], b.inputs['Base Color'])
            if max(tex.size) > 1024:
                f = 1024 / max(tex.size); tex.scale(int(tex.size[0] * f), int(tex.size[1] * f))
        else:
            b.inputs['Base Color'].default_value = [max(c, mini) for c in col[:3]] + [1]
        s.material = nm

def decimer(ob, cible):
    n = len(ob.data.polygons)
    if n > cible:
        d = ob.modifiers.new('d', 'DECIMATE'); d.ratio = cible / n
        bpy.context.view_layer.objects.active = ob
        bpy.ops.object.modifier_apply(modifier='d')

def bornes(obs):
    P = [o.matrix_world @ v.co for o in obs for v in o.data.vertices]
    return [min(p[i] for p in P) for i in range(3)], [max(p[i] for p in P) for i in range(3)]

def transformer(obs, M):
    for o in obs:
        o.data.transform(M)
        if M.determinant() < 0:
            o.data.flip_normals()

def ecrire(ob, nom):
    bpy.ops.object.select_all(action='DESELECT')
    ob.select_set(True)
    bpy.context.view_layer.objects.active = ob
    bpy.ops.object.material_slot_remove_unused()
    bpy.ops.export_scene.gltf(filepath=os.path.join(SORTIE, nom + '.glb'), export_format='GLB', use_selection=True,
        export_yup=False, export_image_format='JPEG', export_animations=False, export_vertex_color='NONE')
    lo, hi = bornes([ob])
    print('PIECE', nom, 'tris', sum(len(p.vertices) - 2 for p in ob.data.polygons), 'materiaux', len(ob.material_slots),
          [round(x, 2) for x in lo], [round(x, 2) for x in hi])

R = lambda a: mathutils.Matrix.Rotation(math.radians(a), 4, 'Z')
T = lambda x, y, z: mathutils.Matrix.Translation((x, y, z))
S = lambda s: mathutils.Matrix.Scale(s, 4)

# --- Leclerc : avant en +X ; caisse 6,88 m.
obs = importer('leclerc/char_leclerc.glb')
obs = [o for o in obs if o.data.materials[0].name not in ('Fake_Glass',)]
tour = [o for o in obs if int(o.name.split('_')[1]) >= 25]
cais = [o for o in obs if o not in tour]
# Les patins de chenille et les galets portent l'essentiel des triangles.
for o in cais + tour:
    decimer(o, {'Object_5': 6000, 'Object_6': 5000, 'Object_23': 5500, 'Object_4': 1200, 'Object_35': 4000, 'Object_32': 2300, 'Object_34': 400}.get(o.name, 1200 if len(o.data.polygons) > 1500 else 400))
lo, hi = bornes(cais)
s = 6.88 / (hi[0] - lo[0])
pivot = (0.35, 0.02)
M = T(0, 0, 0) @ S(s) @ T(-(lo[0] + hi[0]) / 2, -(lo[1] + hi[1]) / 2, -lo[2])
transformer(obs, M)
px = (pivot[0] - (lo[0] + hi[0]) / 2) * s
transformer(tour, T(-px, 0, 0))
print('PIVOT leclerc', round(px, 2))
for l, n in ((cais, 'leclerc_caisse'), (tour, 'leclerc_tourelle')):
    o = joindre(l); mats(o); ecrire(o, n)

# --- VBCI : avant en -Y ; 7,6 m.
obs = importer('vbci/vbci_low_poly.glb')
c = [o for o in obs if o.name.startswith('Object_4')]
t = [o for o in obs if o not in c]
lo, hi = bornes(c)
s = 7.6 / (hi[1] - lo[1])
M = R(90) @ S(s) @ T(-(lo[0] + hi[0]) / 2, -(lo[1] + hi[1]) / 2, -lo[2])
transformer(obs, M)
lt, ht = bornes([o for o in t if len(o.data.polygons) > 300])
px = (lt[0] + ht[0]) / 2
transformer(t, T(-px, 0, 0))
print('PIVOT vbci', round(px, 2))
for l, n in ((c, 'vbci_caisse'), (t, 'vbci_tourelle')):
    o = joindre(l); mats(o); ecrire(o, n)

# --- Rafale : avant en -X ; 15,27 m. Origine au centre.
obs = importer('rafale/dassault_rafale.glb')
lo, hi = bornes(obs)
s = 15.27 / (hi[0] - lo[0])
transformer(obs, R(180) @ S(s) @ T(-(lo[0] + hi[0]) / 2, -(lo[1] + hi[1]) / 2, -(lo[2] + hi[2]) / 2))
for o in obs:
    decimer(o, 2600)
o = joindre(obs); mats(o); ecrire(o, 'rafale')

# --- Charles de Gaulle : 261,5 m ; origine à la flottaison. Axe long = Y à
# l'import ; l'avant est vérifié sur l'image.
obs = importer('charles-de-gaulle/charles_de_gaulle_aircraft_carrier.glb')
lo, hi = bornes(obs)
s = 261.5 / (hi[1] - lo[1])
transformer(obs, R(90) @ S(s) @ T(-(lo[0] + hi[0]) / 2, -(lo[1] + hi[1]) / 2, -lo[2]))
transformer(obs, T(0, 0, -9.0))
o = joindre(obs); mats(o); ecrire(o, 'charles_de_gaulle')

# --- Camion (civil réquisitionné) : avant en -Y.
obs = importer('quaternius-truck/truck2.glb')
lo, hi = bornes(obs)
s = 7.5 / (hi[1] - lo[1])
transformer(obs, R(90) @ S(s) @ T(-(lo[0] + hi[0]) / 2, -(lo[1] + hi[1]) / 2, -lo[2]))
for o in obs: decimer(o, 1500)
o = joindre(obs); mats(o)
# Peint en vert armée : la luminance de l'atlas, teintée.
for s_ in o.material_slots:
    for nd in s_.material.node_tree.nodes:
        if nd.type == 'TEX_IMAGE' and not nd.image.get('peint'):
            px = list(nd.image.pixels)
            for i in range(0, len(px), 4):
                r, g, b = px[i:i + 3]
                l = .3 * r + .59 * g + .11 * b
                if max(r, g, b) > .25 and not (abs(r - g) < .06 and l < .4):
                    px[i:i + 3] = [l * .52, l * .56, l * .38]
            nd.image.pixels = px; nd.image.pack(); nd.image['peint'] = 1
ecrire(o, 'camion')

# --- Arbre : 9 m.
obs = importer('quaternius-tree/tree.glb')
lo, hi = bornes(obs)
s = 9.0 / (hi[2] - lo[2])
transformer(obs, S(s) @ T(-(lo[0] + hi[0]) / 2, -(lo[1] + hi[1]) / 2, -lo[2]))
for o in obs: decimer(o, 700)
o = joindre(obs); mats(o); ecrire(o, 'arbre')
