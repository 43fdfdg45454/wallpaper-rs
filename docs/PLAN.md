# Plan del proyecto

Reemplazo libre de los motores de fondos de pantalla comerciales para Linux y Android: escenas,
vídeo, web, listas, multi-monitor, pausa inteligente, audio reactivo, editor de escenas e
imágenes, servidores propios de contenido y un formato de paquete abierto.

## Principios

- **Formato propio y abierto.** El contenido vive en paquetes `.wpak` documentados
  ([spec](specs/wpak.md)). Nada depende de formatos ni recursos de terceros.
- **Núcleo portable.** Rust y wgpu. Linux y Android comparten el motor.
- **Backends intercambiables** para presentar el fondo, detectar ventanas, capturar audio y
  decodificar vídeo.
- **Seguro por defecto.** Contenido ajeno en procesos aislados, fondos web sin red salvo
  permiso, repositorios firmados, sin telemetría.
- **Distribución local.** Por ahora no se publica en tiendas. GitHub Actions compila y prueba
  cada cambio y deja los paquetes como artefactos descargables.
- **Licencia GPL-3.0-or-later** para todo el código. Recursos propios bajo CC0, CC-BY u OFL.

## Arquitectura

| Componente | Función |
|---|---|
| `wprsd` | Daemon de render por pantalla con wgpu |
| `wprs-ctl` | CLI de control |
| App de escritorio | Biblioteca, repositorios, listas, perfiles, reglas, ajustes |
| Editor | Escenas y editor de imágenes |
| `wprs-server` | Servidor autoalojable de contenido y sincronización |
| App Android | Live wallpaper con el mismo motor |
| Plugin Plasma 6 | Muestra el stream PipeWire del daemon y pausa con ventanas maximizadas |
| Extensión GNOME | Adopta la ventana del daemon como escritorio |

Tecnología: Rust, wgpu, smithay-client-toolkit, x11rb, GStreamer (Linux) y MediaCodec
(Android) para vídeo, WebKitGTK aislado para fondos web, QuickJS para scripts, PipeWire para
audio y streams, zbus para D-Bus, minisign con un esquema tipo TUF para firmas, axum con SQLite
o PostgreSQL para el servidor, Kotlin y Jetpack Compose para la interfaz de Android. La
interfaz de escritorio será egui o GTK4 según una prueba previa de idiomas asiáticos, escritura
de derecha a izquierda y lectores de pantalla.

## Escritorios Linux

| Escritorio | Presentación | Pausa por ventana | Flatpak |
|---|---|---|---|
| KDE Plasma 6 | Stream PipeWire mostrado por el plugin con KPipeWire | TasksModel en el plugin | Sí |
| GNOME 50+ | Ventana del daemon adoptada como escritorio por la extensión | La extensión | Sí |
| Sway, Hyprland, niri, COSMIC | layer-shell en la capa de fondo | foreign-toplevel | No: los compositores ocultan esos protocolos a apps en sandbox; se usa la instalación nativa |
| X11 sin gestor de escritorio | Ventana de tipo escritorio | EWMH | Sí |

## Hitos

Cada hito llega como uno o varios pull requests con pruebas automáticas, y se valida después
en hardware real.

| Hito | Tamaño | Entrega |
|---|---|---|
| **H0 Fundaciones** | Pequeño | Workspace, CI, licencia, specs v0 (`.wpak`, índice de repositorios, D-Bus). Daemon que renderiza una imagen con wgpu y la publica por PipeWire. Plugin mínimo de Plasma 6. CLI. Instalación de usuario. |
| H1 Reproducción | Mediano | Vídeo con GStreamer, transiciones, multi-monitor completo, pausa por ventana con TasksModel, batería y bloqueo, arranque automático. |
| H2 App y contenido | Mediano | App de escritorio: biblioteca, listas, perfiles, presets, ajustes. Validador `.wpak`. Repositorios estáticos firmados. Proveedores de imágenes: carpeta local, WebDAV/Nextcloud, Immich, Wallhaven, Bing, NASA APOD, Wikimedia Commons. |
| H3 Editor de imágenes y Flatpak | Mediano | Recorte por pantalla, color, máscaras, mapa de profundidad. Paquete Flatpak local. Backends wlroots y X11. |
| H4 Motor de escenas | Grande | Capas, 30 efectos, partículas, líneas de tiempo, propiedades de usuario, audio reactivo, cursor, scripts. |
| H5 Servidor propio | Mediano | Catálogo, subidas, avisos legales mínimos, buzón de sincronización, imagen Docker. |
| H6 Android | Grande | Live wallpaper con imagen, vídeo y escenas, listas, giroscopio, Material You, sincronización por red local y por servidor. APK de depuración. |
| H7 Editor de escenas | Muy grande | Capas, efectos con máscaras, partículas, línea de tiempo, scripts, exportar y publicar. |
| H8 Paridad avanzada | Grande | Fondos web, pantalla de bloqueo, extensión GNOME, reglas por aplicación, salvapantallas, efectos restantes. |
| H9 Importación de Wallpaper Engine (secundaria) | Mediano | Conversor opcional, ver abajo. |

Después: modelos 3D, puppet warp, fondos de tipo aplicación, HDR, RGB, federación de servidores.

### H9: importación de Wallpaper Engine

Es una tarea secundaria y opcional: un conversor que toma un fondo de Wallpaper Engine que el
usuario ya tiene y lo transforma en un paquete `.wpak` propio. No hay compatibilidad directa
ni ejecución de su contenido:

- Los efectos, shaders y recursos internos de Wallpaper Engine son propietarios y no se
  redistribuyen. El conversor los sustituye por equivalentes propios y avisa de lo que no puede
  reproducir.
- El contenido del Workshop pertenece a sus autores. La conversión es para uso personal: los
  paquetes convertidos se marcan con su origen y la app no los publica en repositorios.
- Alcance previsto: vídeo e imagen primero, luego escenas sencillas (capas, texto, efectos
  comunes). Los shaders requieren un compilador propio de su dialecto, por eso queda al final.

## Riesgos

| Riesgo | Mitigación |
|---|---|
| Flatpak en compositores wlroots | Instalación nativa de usuario; diagnóstico en la app |
| Cambios de GNOME Shell entre versiones | Soportar las dos últimas versiones |
| Vídeo sin copias en wgpu | Ruta con copia siempre disponible; ruta GLES/EGL |
| Términos de proveedores de imágenes | Solo proveedores compatibles; crédito siempre visible |
| Tamaño del editor | Entrega por capas; esquema de datos compartido con el motor |
| Batería en Android | Vídeo directo a la superficie, 30 FPS en batería, pausa agresiva |
