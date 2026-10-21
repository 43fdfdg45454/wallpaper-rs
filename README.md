# wallpaper-rs

Fondos de pantalla para Linux, libres y escritos en Rust.

Un daemon dibuja el fondo de cada pantalla con la GPU y KDE Plasma 6 lo muestra a través
de un plugin de fondo.

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
- `wprs-ctl` controla el daemon desde la terminal.

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
systemd, activación por D-Bus y el plugin de Plasma. Funciona también en distribuciones con
sistema de archivos inmutable. Para desinstalar: `./uninstall.sh` (añade `--purge` para borrar
también la configuración).

Después, en Plasma: clic derecho en el escritorio → *Configurar escritorio y fondo de
pantalla* → tipo de fondo **wallpaper-rs** → elegir imagen.

Requisitos: KDE Plasma 6, PipeWire y un controlador Vulkan u OpenGL.

## Uso desde la terminal

```sh
wprs-ctl set ~/Imágenes/fondo.jpg               # todas las pantallas
wprs-ctl set ~/Imágenes/otro.png -o DP-1 -f fit # una pantalla, modo de ajuste
wprs-ctl state                                  # estado en JSON
wprs-ctl reload                                 # releer la configuración
```

Modos de ajuste: `fill`, `fit`, `stretch`, `center`, `tile`. La configuración vive en
`~/.config/wallpaper-rs/config.toml`.

## Desarrollo

Dependencias de compilación (Debian/Ubuntu): `libpipewire-0.3-dev libclang-dev pkg-config`.
Para las pruebas: `mesa-vulkan-drivers pipewire wireplumber dbus-x11`.

```sh
cargo test --workspace                   # pruebas unitarias y de render (GPU por software)
cargo build -p wprs-pipewire --example probe
cargo build -p wprsd -p wprs-ctl
cp target/debug/examples/probe target/debug/
scripts/integration-test.sh target/debug # D-Bus + PipeWire + daemon de punta a punta
scripts/package.sh                       # genera dist/wallpaper-rs-*.tar.gz
```

La CI ejecuta además `scripts/plasma-test.sh` en un contenedor con Plasma 6: instala el
paquete con `install.sh`, arranca una sesión Wayland sin pantalla (KWin y plasmashell sobre
dispositivos DRM virtuales), activa el plugin y comprueba los píxeles de una captura.

| Ruta | Contenido |
|------|-----------|
| `crates/wprs-core` | Configuración, modos de ajuste, nombres D-Bus |
| `crates/wprs-render` | Renderizador wgpu sin ventana |
| `crates/wprs-pipewire` | Exportación de streams de vídeo PipeWire |
| `apps/wprsd` | Daemon |
| `apps/wprs-ctl` | CLI |
| `integrations/plasma-wallpaper` | Plugin de fondo de Plasma 6 |
| `packaging` | Servicio systemd, activación D-Bus, scripts de instalación |
| `docs` | Especificación de la interfaz D-Bus |

## Licencia

GPL-3.0-or-later. Ver [`LICENSE`](LICENSE).
