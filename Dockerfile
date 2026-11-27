# syntax=docker/dockerfile:1
# Assets du jeu : téléchargement (`assets telecharger`) puis conversion
# (`assets convertir`) dans un Job du chart `game`. Bases durcies de Docker
# (dhi.io), épinglées par digest ; exécution non root, sans gestionnaire de
# paquets. Le manifeste est compilé dans `assets` : le digest de l'image fixe
# les assets d'une publication, comme pour la chaîne de données (H-06).

FROM dhi.io/rust:1.100.0-debian13-dev@sha256:70b12263cf778cba39a48563b13b55d08ef4c7db5df648d1e1a87c398e1f5f48 AS construction
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    cargo build --release --locked --bins \
 && install -D target/release/assets /opt/assets/assets

# Blender officiel (sorties identiques à P-26 avec la 5.3.0), vérifié par le
# SHA-256 publié sur download.blender.org. Même sans fenêtre (`-b`), il charge
# X11, xkbcommon, Vulkan et GL : paquets du dépôt de DHI, versions exactes
# (init-system-helpers : `update-rc.d`, appelé par x11-common).
FROM dhi.io/debian-base:trixie-dev@sha256:120051dda16c2611454d2c54a0959f23f80c32493d0e528a5d1348bca82dfbc8 AS blender
ARG BLENDER_VERSION=5.3.0
ARG BLENDER_SHA256=dacb8264197a3f7fda6001285a5a506fc97ee943881261c2c3bd63c882f22e05
SHELL ["/bin/bash", "-euo", "pipefail", "-c"]
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      init-system-helpers=1.69~deb13u1+dhi1 \
      ca-certificates=20250419+deb13u1+dhi0 \
      curl=8.14.1-2+deb13u7+dhi1 \
      xz-utils=5.8.1-1+deb13u2+dhi0 \
      libx11-6=2:1.8.12-1+deb13u0+dhi0 \
      libxrender1=1:0.9.12-1+deb13u0+dhi1 \
      libxfixes3=1:6.0.0-2+deb13u0+dhi1 \
      libxi6=2:1.8.2-1+deb13u0+dhi1 \
      libxkbcommon0=1.7.0-2+deb13u0+dhi0 \
      libvulkan1=1.4.309.0-1+deb13u0+dhi0 \
      libxcb1=1.17.0-2+deb13u0+dhi3 \
      libxext6=2:1.3.4-1+deb13u0+dhi1 \
      libsm6=2:1.2.6-1+deb13u0+dhi0 \
      libice6=2:1.1.1-1+deb13u0+dhi0 \
      libgl1=1.7.0-1+deb13u0+dhi1 \
      libxau6=1:1.0.11-1+deb13u0+dhi0 \
      libxdmcp6=1:1.1.5-1+deb13u0+dhi0 \
      libuuid1=2.41.5-0+deb13u1+dhi3 \
      libstdc++6=14.2.0-19+deb13u0+dhi9 \
 && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL --proto '=https' -o /tmp/blender.tar.xz \
      "https://download.blender.org/release/Blender${BLENDER_VERSION%.*}/blender-${BLENDER_VERSION}-linux-x64.tar.xz" \
 && echo "${BLENDER_SHA256}  /tmp/blender.tar.xz" | sha256sum -c - \
 && mkdir -p /opt/blender \
 && tar -xJf /tmp/blender.tar.xz --strip-components=1 -C /opt/blender \
 && rm /tmp/blender.tar.xz
# Paquets Python embarqués par Blender et inutiles aux scripts de conversion
# (bpy, mathutils et bibliothèque standard seulement) : pip, setuptools et
# leurs copies embarquées, client HTTP. Retirés : ils portent des
# vulnérabilités connues (trivy) et ne servent qu'aux extensions en ligne.
RUN shopt -s nullglob \
 && sp=$(echo /opt/blender/[0-9]*/python/lib/python3*/site-packages) \
 && rm -rf "$sp"/{pip,pip-*.dist-info,setuptools,setuptools-*.dist-info,pkg_resources,_distutils_hack} \
      "$sp"/{distutils-precedence.pth,requests,requests-*.dist-info,urllib3,urllib3-*.dist-info} \
      "$sp"/{certifi,certifi-*.dist-info,idna,idna-*.dist-info,charset_normalizer,charset_normalizer-*.dist-info} \
 && test -d "$sp/numpy" && ! test -e "$sp/pip" && ! test -e "$sp/urllib3"
# Bibliothèques système dont Blender et ses modules dépendent, hors glibc
# (fournie par l'image d'exécution) : copiées dans /opt/lib-sys.
RUN mkdir -p /opt/lib-sys \
 && find /opt/blender -type f \( -name blender -o -name '*.so' -o -name '*.so.*' \) -print0 \
  | xargs -0 -n 50 ldd 2>/dev/null \
  | awk '$2 == "=>" && $3 ~ /^\// {print $3}' | sort -u \
  | grep -v -E '^/opt/blender/' \
  | grep -v -E '/(libc|libm|libmvec|libdl|librt|libpthread|libutil|libresolv|libgcc_s|ld-linux-x86-64)\.so' \
  | xargs -r cp -L -t /opt/lib-sys/ \
 && ls /opt/lib-sys

FROM dhi.io/debian-base:trixie@sha256:ad31467223560f8eb2fd60e983be09bc04c2050175555bac2c22b3ca72c24b39
COPY --from=blender /opt/blender /opt/blender
COPY --from=blender /opt/lib-sys /opt/lib-sys
COPY --from=construction /opt/assets/assets /opt/assets/assets
COPY blender/ /opt/assets/blender/
# Blender écrit sa configuration sous $HOME : /tmp, un emptyDir dans le Job.
ENV LD_LIBRARY_PATH=/opt/lib-sys HOME=/tmp
# Utilisateur « nonroot » de l'image de base, explicite.
USER 65532:65532
ENTRYPOINT ["/opt/assets/assets"]
