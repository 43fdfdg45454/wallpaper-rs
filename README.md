# wallpaper-rs

Fondos de pantalla para Linux, libres y escritos en Rust: imágenes y vídeos en bucle, con
transiciones, un fondo distinto por pantalla y pausa automática cuando no se ven.

Un daemon dibuja el fondo de cada pantalla con la GPU y KDE Plasma 6 lo muestra a través
de un plugin de fondo; en Sway, Hyprland, river, niri y COSMIC el daemon lo dibuja él mismo. Todo
se gestiona desde la app de escritorio, desde la terminal (`wprs-ctl`) o desde el icono de la
bandeja del sistema.

## Cómo funciona

```
 Plasma (plugin de fondo, QML)                    wprsd (daemon, Rust)
 ┌──────────────────────────┐   D-Bus             ┌───────────────────────────┐
 │ RequestStream(pantalla,  │ ──────────────────► │ renderiza con wgpu        │
 │   ancho, alto)           │ ◄── nodo, serial ── │ exporta un stream PipeWire│
 │ PipeWireSourceItem       │ ◄══ vídeo PipeWire ═│ por pantalla              │
 └──────────────────────────┘                     └───────────────────────────┘
                                   wprs-ctl (CLI) ──► D-Bus
```

- `wprsd` renderiza en la GPU sin ventana propia y publica un stream PipeWire por pantalla.
- El plugin de Plasma pide un stream del tamaño exacto de su pantalla y lo muestra con
  KPipeWire, sin tapar iconos ni widgets del escritorio.
- `wprs-ctl` y la app de escritorio controlan el daemon: cada acción está en los dos. El icono
  de la bandeja ofrece las más frecuentes y abre la app.
- Los vídeos se decodifican con GStreamer y se pausan cuando una ventana maximizada o a
  pantalla completa tapa la pantalla, con batería o con la pantalla bloqueada.

## Instalación (sin root)

Cada ejecución de la CI en GitHub Actions genera el artefacto `wallpaper-rs-x86_64-linux`.
Descárgalo desde la pestaña *Actions* (es un `.zip` con un `.tar.gz` dentro, que conserva los
permisos de ejecución) y ejecuta:

```sh
unzip wallpaper-rs-x86_64-linux.zip
tar xzf wallpaper-rs-*-x86_64-linux.tar.gz
cd wallpaper-rs-*-x86_64-linux
./install.sh
```

Instala todo en tu directorio personal: binarios en `~/.local/bin`, servicio de usuario de
systemd, activación por D-Bus, la app de escritorio (con su lanzador e icono), el plugin de fondo
y el icono de la bandeja de Plasma. Funciona también en distribuciones con
sistema de archivos inmutable. Para desinstalar: `./uninstall.sh` (añade `--purge` para borrar
también la configuración).

Después, en Plasma: clic derecho en el escritorio → *Configurar escritorio y fondo de
pantalla* → tipo de fondo **wallpaper-rs**. El fondo de cada pantalla, el modo de ajuste, la
pausa, el volumen y los ajustes se eligen desde el icono de wallpaper-rs en la bandeja.

Requisitos: KDE Plasma 6.4 o posterior, PipeWire, GStreamer (con los plugins base y good) y un
controlador Vulkan u OpenGL. El plugin de fondo usa los módulos QML de KPipeWire, KWindowSystem y
plasma-workspace; en Debian y Ubuntu vienen en `qml6-module-org-kde-pipewire`,
`qml6-module-org-kde-kwindowsystem` y `plasma-workspace`. La app de escritorio usa Qt 6 Quick,
Kirigami y el estilo de escritorio de KDE (`qml6-module-org-kde-kirigami` y
`qml6-module-org-kde-qqc2desktopstyle`).

## Instalación con Flatpak

La CI genera también el artefacto `wallpaper-rs-flatpak`, con el daemon, `wprs-ctl`, GStreamer y
los códecs dentro de un Flatpak. El runtime (`org.freedesktop.Platform` 26.08) se descarga de
Flathub al instalarlo:

```sh
unzip wallpaper-rs-flatpak.zip
flatpak install --user ./wallpaper-rs.flatpak
flatpak run --command=wprs-plasma io.github.wallpaper_rs.WallpaperRs | sh
```

Flatpak no puede instalar el plugin de fondo ni el icono de la bandeja de Plasma: la última orden
los instala con `kpackagetool6` desde los archivos del propio Flatpak (sin `| sh` solo muestra las
órdenes). Hay que repetirla después de cada actualización y reiniciar Plasma. El plugin arranca el
daemon por D-Bus cuando lo necesita; no hay servicio de systemd. No instales a la vez el Flatpak y
el `.tar.gz`, porque los dos registran el mismo servicio D-Bus: ejecuta antes `./uninstall.sh`.

```sh
alias wprs-ctl='flatpak run --command=wprs-ctl io.github.wallpaper_rs.WallpaperRs'
```

Permisos: la GPU, el socket de PipeWire para publicar los streams, el sonido de los vídeos, el
nombre D-Bus del daemon, el bloqueo de pantalla y la batería, el llavero del sistema (Secret
Service) para las contraseñas y claves de los proveedores, lectura de Imágenes, Vídeos y
Descargas, y red. La red solo se usa para los proveedores de fondos y los repositorios que
añadas tú. Para usar fondos de otra carpeta:
`flatpak override --user --filesystem=/ruta:ro io.github.wallpaper_rs.WallpaperRs`. La
configuración vive en `~/.var/app/io.github.wallpaper_rs.WallpaperRs/config/wallpaper-rs/`. Para
desinstalar: `flatpak run --command=wprs-plasma io.github.wallpaper_rs.WallpaperRs remove | sh` y
`flatpak uninstall --user io.github.wallpaper_rs.WallpaperRs`.

## Sway, Hyprland, river, niri y COSMIC

Aquí no hace falta plugin: `wprsd` pone en cada pantalla una superficie en la capa de fondo
(`wlr-layer-shell`) y dibuja en ella con la GPU, sin copias. Se instala con el `.tar.gz` y se
arranca desde la configuración del compositor:

```sh
exec wprsd                       # Sway (~/.config/sway/config)
exec-once = wprsd                # Hyprland (~/.config/hypr/hyprland.conf)
riverctl spawn wprsd             # river (~/.config/river/init)
spawn-at-startup "wprsd"         # niri (~/.config/niri/config.kdl)
```

En COSMIC, añádelo a las aplicaciones de inicio y elige un color sólido como fondo propio del
escritorio. Si `install.sh` dejó activo el servicio de usuario, pásale el entorno gráfico desde la
misma configuración en vez de lanzar el binario: `systemctl --user import-environment
WAYLAND_DISPLAY && systemctl --user restart wprsd` (en Hyprland, importa también
`HYPRLAND_INSTANCE_SIGNATURE`).

`wprsd` elige solo: con Plasma (`plasmashell` en el bus o `KDE` en `XDG_CURRENT_DESKTOP`) usa los
streams del plugin y en otro escritorio Wayland con `wlr-layer-shell`, sus propias superficies.
`wprsd --backend streams|layer-shell` lo fuerza. Las pantallas se llaman como las anuncia el
compositor (`DP-1`, `HDMI-A-1`…; las muestran `swaymsg -t get_outputs`, `hyprctl monitors`,
`niri msg outputs` o `wlr-randr`) y esos nombres son los de `-o` y de la configuración. Las
pantallas que se conectan o se quitan con la sesión abierta se siguen solas, y la escala
fraccional se respeta.

Los vídeos se pausan cuando una ventana tapa la pantalla: el compositor deja de pedir fotogramas
al fondo, una ventana activa a pantalla completa o maximizada lo confirma y, en Hyprland, también
su IPC. El Flatpak no sirve en estos escritorios: no dejan usar `wlr-layer-shell` ni la
información de ventanas a las aplicaciones aisladas.

## Uso desde la terminal

```sh
wprs-ctl set ~/Imágenes/fondo.jpg               # todas las pantallas
wprs-ctl set ~/Vídeos/mar.mp4 -o DP-1 -f fit    # una pantalla, modo de ajuste
wprs-ctl fit center                             # solo el modo de ajuste, mismo fondo
wprs-ctl fit tile -o DP-1                       # el modo de ajuste de una pantalla
wprs-ctl background '#203040'                   # color alrededor del fondo
wprs-ctl pause | resume | toggle                # pausar los fondos animados
wprs-ctl volume 40 | mute | unmute              # sonido de los vídeos
wprs-ctl config                                 # ver los ajustes
wprs-ctl config transition.duration_ms 1000     # cambiar un ajuste
wprs-ctl state                                  # estado en JSON
wprs-ctl reload                                 # releer la configuración
```

Modos de ajuste: `fill`, `fit`, `stretch`, `center`, `tile` y `span` (una imagen sobre todas las
pantallas, con su tamaño físico y los marcos entre ellas). Ajustes: transición (`fade` o
`none` y su duración), fotogramas por segundo y cuándo pausar (ventana encima, batería,
bloqueo). Un ajuste desconocido termina con error y muestra la lista de ajustes válidos.

La configuración vive en `~/.config/wallpaper-rs/config.toml` y solo la puede leer su usuario.
Si se edita a mano, los valores fuera de rango se corrigen al leerla. Si tiene un error de
sintaxis, el daemon arranca con los valores por defecto y muestra el error en `wprs-ctl state`
(`config_error`); el archivo roto se copia a `config.toml.broken` antes de guardar otro.

Para pruebas, `wprs-ctl stream <salida> <ancho> <alto>` crea un flujo que sigue vivo al salir
la orden y `wprs-ctl release <salida>` lo destruye. Con `--wait`, el flujo dura lo que dure la
orden, igual que el de un escritorio.

La biblioteca, las listas, los perfiles, los proveedores y los paquetes se manejan igual
(`wprs-ctl <grupo> --help` muestra cada orden):

```sh
wprs-ctl lib add ~/Imágenes --wait              # añadir una carpeta y esperar al escaneo
wprs-ctl lib search 'tag:mar sort:random'       # buscar; los elementos se nombran por su uid
wprs-ctl lib apply 1R1AB2 -o DP-1               # mostrar un elemento (uid o sus últimos 6+)
wprs-ctl playlist new Mar --query tag:mar       # lista inteligente
wprs-ctl playlist start Mar && wprs-ctl next    # rotar una lista y avanzar
wprs-ctl profile save Escritorio                # guardar lo que muestra cada pantalla
wprs-ctl provider add diario bing               # una fuente en línea
wprs-ctl remote add 'https://example.org/repo/?fingerprint=…'
wprs-ctl pkg install org.example.dawn --wait    # instalar un paquete de un repositorio
wprs-ctl jobs | watch | schema                  # trabajos, cambios en vivo, claves válidas
```

Las consultas aceptan `--json` e imprimen la respuesta del daemon tal cual; sin él muestran
tablas para leer, no para scripts. Lo que lanza un trabajo imprime su número y termina; con
`--wait` muestra el progreso y el resultado, y Ctrl-C deja de esperar sin parar el trabajo
(`wprs-ctl cancel <n>` lo para). Las contraseñas y claves de los proveedores se piden sin eco o
se leen de la primera línea de la entrada con `--secret-stdin`; nunca van en la orden. Códigos
de salida: 0 bien, 1 error, 2 uso incorrecto.

Las ediciones no tocan la imagen: se guardan en `<imagen>.wprs.json` a su lado (o en
`~/.local/share/wallpaper-rs/edits/` si la carpeta no se puede escribir), las aplica la GPU y el
daemon las vuelve a leer en cuanto cambian. `wprs-ctl edit` las cambia sin daemon:

```sh
wprs-ctl edit set foto.jpg ev=0.3 contrast=10 temperature=5200 tint=4
wprs-ctl edit crop foto.jpg 0.1,0.05,0.8,0.9 --rotate 90
wprs-ctl edit overlay foto.jpg '#ff8800' --blend soft-light --opacity 0.3 --mask linear:0.5,0,0.5,0.5
wprs-ctl edit preview foto.jpg ev=1                  # verlo en el escritorio 60 s sin guardar
wprs-ctl edit export foto.jpg -o foto-editada.png    # mismo resultado, sin daemon
wprs-ctl set foto.jpg -f span && wprs-ctl edit span foto.jpg --zoom 1.2
```

`wprs-ctl pack` (crear, firmar y validar paquetes `.wpak`) y `wprs-ctl repo` (claves, firma,
publicación, rotación y verificación de un repositorio) funcionan sin daemon. La contraseña de
una clave se pide en la terminal o se lee de `WPRS_KEY_PASSWORD`.

## Desarrollo

Dependencias de compilación (Debian/Ubuntu): `libpipewire-0.3-dev libclang-dev pkg-config
libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev qt6-base-dev qt6-declarative-dev`. Para las
pruebas: `mesa-vulkan-drivers pipewire wireplumber dbus-x11 gstreamer1.0-plugins-base
gstreamer1.0-plugins-good`, y `sway grim foot` para la de Sway.

```sh
cargo test --workspace                   # pruebas unitarias, de render y de vídeo
cargo build -p wprs-pipewire --example probe
cargo build -p wprsd -p wprs-ctl
cp target/debug/examples/probe target/debug/
scripts/integration-test.sh target/debug # D-Bus + PipeWire + daemon de punta a punta
scripts/sway-test.sh target/debug        # superficies layer-shell en un Sway sin pantalla
scripts/package.sh                       # genera dist/wallpaper-rs-*.tar.gz
scripts/translations.sh update [idioma]  # actualiza po/ con los textos del QML
target/debug/wprs-app --check            # carga todo el QML de la app y falla ante cualquier aviso
```

Flatpak local (compila sin red, con Rust 1.99.1 y las fuentes de Cargo descargadas antes por
`flatpak-builder`):

```sh
packaging/flatpak/cargo-sources.py      # genera cargo-sources.json desde Cargo.lock
flatpak-builder --user --install --install-deps-from=flathub --force-clean \
  --state-dir=target/flatpak-builder target/flatpak \
  packaging/flatpak/io.github.wallpaper_rs.WallpaperRs.yml
```

La CI ejecuta además `scripts/plasma-test.sh` en un contenedor con Plasma 6: instala el
paquete con `install.sh`, arranca una sesión Wayland sin pantalla (KWin y plasmashell sobre
dispositivos DRM virtuales), activa el plugin y comprueba los píxeles de una captura. También
ejecuta `scripts/sway-test.sh` en un Sway sin pantalla: captura cada salida con `grim`, conecta y
quita una pantalla y comprueba que el vídeo se pausa bajo una ventana a pantalla completa y con
la pantalla apagada.

| Ruta | Contenido |
|------|-----------|
| `crates/wprs-core` | Configuración, modos de ajuste, nombres D-Bus |
| `crates/wprs-render` | Renderizador wgpu, sin ventana o sobre superficies Wayland |
| `crates/wprs-pipewire` | Exportación de streams de vídeo PipeWire |
| `crates/wprs-video` | Reproducción de vídeo en bucle con GStreamer |
| `crates/wprs-wayland` | Superficies layer-shell, escala, ritmo de fotogramas y oclusión en Wayland |
| `apps/wprsd` | Daemon |
| `apps/wprs-ctl` | CLI |
| `apps/wprs-app` | App de escritorio (Qt Quick y Kirigami sobre Rust con cxx-qt) |
| `integrations/plasma-wallpaper` | Plugin de fondo de Plasma 6 |
| `integrations/plasma-tray` | Icono de gestión en la bandeja de Plasma 6 |
| `po` | Traducciones gettext de los paquetes de Plasma y de la app |
| `packaging` | Servicio systemd, activación D-Bus, lanzador e icono, scripts de instalación, Flatpak |
| `docs` | Especificación de la interfaz D-Bus |

## Privacidad

- Sin telemetría ni estadísticas de uso. El servicio solo se conecta a internet para los
  proveedores de fondos y los repositorios que añadas tú. Al arrancar solo contacta con un
  proveedor si le toca su sincronización automática; la comprobación diaria de los repositorios
  (`repos.auto_refresh`) empieza 10 minutos después.
- Los proveedores de fondos en línea (Bing, APOD, Wikimedia, Wallhaven, WebDAV/Nextcloud, Immich)
  solo se usan si los configuras. Los servicios públicos se usan siempre por HTTPS; todos llevan un
  User-Agent genérico (`wallpaper-rs/<versión>`), sin cookies ni cabecera Referer, y las claves de API
  y contraseñas solo se envían al servidor configurado. Se guardan en el llavero del sistema
  (Secret Service) y en ningún otro sitio: ni en la configuración, ni en los registros, ni en la
  línea de órdenes. `http://` sin cifrar a otro equipo solo se permite con `allow_plain_http`.
- La configuración se escribe con permisos `0600` dentro de una carpeta `0700`: solo tu usuario
  puede leerla.
- La caché de descargas se guarda en una carpeta privada.
- La biblioteca y las copias importadas (`~/.local/share/wallpaper-rs`), las miniaturas y las
  entradas de paquetes (`~/.cache/wallpaper-rs`) se guardan con permisos `0600` en carpetas `0700`.
  Las miniaturas y el análisis de la biblioteca (tamaño, duración, color) los hace un proceso aparte
  con la prioridad más baja de CPU y disco.
- El servicio de systemd solo puede abrir conexiones de red (IPv4 e IPv6) para los proveedores y
  repositorios que añadas tú, y sus registros no incluyen rutas, nombres de archivo, servidores
  propios ni el modelo de GPU salvo con `RUST_LOG=debug`.
- Los paquetes se generan sin rutas, usuarios ni nombre del equipo de quien los compila.

## Licencia

GPL-3.0-or-later. Ver [`LICENSE`](LICENSE).
