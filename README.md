# wallpaper-rs

Fondos de pantalla para Linux, libres y escritos en Rust: imágenes y vídeos en bucle, con
transiciones, un fondo distinto por pantalla y pausa automática cuando no se ven.

Un daemon dibuja el fondo de cada pantalla con la GPU y KDE Plasma 6 lo muestra a través
de un plugin de fondo. Todo se gestiona igual desde la terminal (`wprs-ctl`) o desde el icono
de la bandeja del sistema.

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
- `wprs-ctl` y el icono de la bandeja controlan el daemon: cada acción está en los dos.
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
systemd, activación por D-Bus, el plugin de fondo y el icono de la bandeja de Plasma. Funciona también en distribuciones con
sistema de archivos inmutable. Para desinstalar: `./uninstall.sh` (añade `--purge` para borrar
también la configuración).

Después, en Plasma: clic derecho en el escritorio → *Configurar escritorio y fondo de
pantalla* → tipo de fondo **wallpaper-rs**. El fondo de cada pantalla, el modo de ajuste, la
pausa, el volumen y los ajustes se eligen desde el icono de wallpaper-rs en la bandeja.

Requisitos: KDE Plasma 6.4 o posterior, PipeWire, GStreamer (con los plugins base y good) y un
controlador Vulkan u OpenGL.

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

Modos de ajuste: `fill`, `fit`, `stretch`, `center`, `tile`. Ajustes: transición (`fade` o
`none` y su duración), fotogramas por segundo y cuándo pausar (ventana encima, batería,
bloqueo). Un ajuste desconocido termina con error y muestra la lista de ajustes válidos.

La configuración vive en `~/.config/wallpaper-rs/config.toml` y solo la puede leer su usuario.
Si se edita a mano, los valores fuera de rango se corrigen al leerla. Si tiene un error de
sintaxis, el daemon arranca con los valores por defecto y muestra el error en `wprs-ctl state`
(`config_error`); el archivo roto se copia a `config.toml.broken` antes de guardar otro.

Para pruebas, `wprs-ctl stream <salida> <ancho> <alto>` crea un flujo que sigue vivo al salir
la orden y `wprs-ctl release <salida>` lo destruye. Con `--wait`, el flujo dura lo que dure la
orden, igual que el de un escritorio.

## Desarrollo

Dependencias de compilación (Debian/Ubuntu): `libpipewire-0.3-dev libclang-dev pkg-config
libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev`. Para las pruebas: `mesa-vulkan-drivers
pipewire wireplumber dbus-x11 gstreamer1.0-plugins-base gstreamer1.0-plugins-good`.

```sh
cargo test --workspace                   # pruebas unitarias, de render y de vídeo
cargo build -p wprs-pipewire --example probe
cargo build -p wprsd -p wprs-ctl
cp target/debug/examples/probe target/debug/
scripts/integration-test.sh target/debug # D-Bus + PipeWire + daemon de punta a punta
scripts/package.sh                       # genera dist/wallpaper-rs-*.tar.gz
scripts/translations.sh update [idioma]  # actualiza po/ con los textos del QML
```

La CI ejecuta además `scripts/plasma-test.sh` en un contenedor con Plasma 6: instala el
paquete con `install.sh`, arranca una sesión Wayland sin pantalla (KWin y plasmashell sobre
dispositivos DRM virtuales), activa el plugin y comprueba los píxeles de una captura.

| Ruta | Contenido |
|------|-----------|
| `crates/wprs-core` | Configuración, modos de ajuste, nombres D-Bus |
| `crates/wprs-render` | Renderizador wgpu sin ventana |
| `crates/wprs-pipewire` | Exportación de streams de vídeo PipeWire |
| `crates/wprs-video` | Reproducción de vídeo en bucle con GStreamer |
| `apps/wprsd` | Daemon |
| `apps/wprs-ctl` | CLI |
| `integrations/plasma-wallpaper` | Plugin de fondo de Plasma 6 |
| `integrations/plasma-tray` | Icono de gestión en la bandeja de Plasma 6 |
| `po` | Traducciones gettext de los paquetes de Plasma |
| `packaging` | Servicio systemd, activación D-Bus, scripts de instalación |
| `docs` | Especificación de la interfaz D-Bus |

## Privacidad

- Sin telemetría ni estadísticas de uso. El servicio no se conecta a internet: los fondos se leen
  de archivos locales.
- Los proveedores de fondos en línea (Bing, APOD, Wikimedia, Wallhaven, WebDAV/Nextcloud, Immich)
  solo se usan si los configuras. Los servicios públicos se usan siempre por HTTPS; todos llevan un
  User-Agent genérico (`wallpaper-rs/<versión>`), sin cookies ni cabecera Referer, y las claves de API
  y contraseñas solo se envían al servidor configurado.
- La caché de descargas se guarda en una carpeta privada.
- Los paquetes se generan sin rutas, usuarios ni nombre del equipo de quien los compila.

## Licencia

GPL-3.0-or-later. Ver [`LICENSE`](LICENSE).
