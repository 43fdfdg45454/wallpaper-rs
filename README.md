# wallpaper-rs

Fondos de pantalla para Linux, libres y escritos en Rust: imágenes y vídeos en bucle, con
transiciones, un fondo distinto por pantalla y pausa automática cuando no se ven.

Un daemon dibuja el fondo de cada pantalla con la GPU y KDE Plasma 6 lo muestra a través
de un plugin de fondo; en Sway, Hyprland, river, niri, COSMIC y las sesiones X11 el daemon lo
dibuja él mismo. Todo se gestiona desde la app de escritorio, desde la terminal (`wprs-ctl`) o
desde el icono de la bandeja del sistema.

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
  pantalla completa tapa la pantalla, con batería o con la pantalla bloqueada; con la pantalla
  bloqueada no suenan nunca, aunque no se pausen.

## Versiones

Las versiones siguen Semantic Versioning 2.0.0 de forma estricta. GitVersion las calcula en la CI
a partir de los commits de la rama principal, escritos con Conventional Commits
(`tipo(ámbito)!: descripción`, donde el ámbito y `!` son opcionales):

- `fix` y `perf` suben el parche: 0.3.0 → 0.3.1.
- `feat` sube la versión menor: 0.3.0 → 0.4.0.
- `!` tras el tipo o el ámbito, o un pie `BREAKING CHANGE:` o `BREAKING-CHANGE:`, sube la mayor,
  también de 0.x a 1.0.0.
- `build`, `chore`, `ci`, `docs`, `refactor`, `revert`, `style` y `test` no cambian la versión.

La CI comprueba el asunto de todos los commits desde la última versión publicada y el título de
cada pull request, también cada vez que se edita; mientras alguno no siga ese formato, la rama
principal o la pull request fallan y no se publica nada.
Un commit mal escrito que ya está en la rama se corrige añadiendo su hash a `ignore.sha` en
`GitVersion.yml` junto con un commit nuevo del tipo correcto. GitVersion toma como actual el commit
de fecha más reciente, así que cada commit necesita una fecha de commit posterior a la de toda su
historia, algo que la CI también comprueba. Si la fecha se fija a mano, sirve
`GIT_COMMITTER_DATE=@$(( $(git log --format=%ct | sort -n | tail -1) + 1 ))`.

Solo los commits que suben la versión publican una nueva: cuando el último commit de la rama
principal pasa todas las pruebas y su versión es mayor que todas las publicadas, la CI crea
`v<versión>` en la página *Releases* del repositorio, con los archivos, un SBOM SPDX de las
dependencias (`wallpaper-rs.spdx.json`), un `SHA256SUMS` firmado y las notas agrupadas en cambios
incompatibles, novedades y correcciones, y publica la imagen firmada del servidor propio. La firma
y la imagen solo se hacen cuando el repositorio está en la organización `wallpaper-rs`. Una
versión publicada no cambia nunca y volver a lanzar una ejecución antigua no publica nada. Las
demás ramas y las pull requests llevan una prerelease de la versión siguiente cuyo número crece con
cada commit (`0.3.1-branch-rama.2`, `0.4.0-feature-rama.1`, `0.3.1-PullRequest12.1`) y no se
publican. `--version`, el daemon por D-Bus, el servidor y los paquetes de Plasma y KWin muestran la
versión; una compilación local sin `WPRS_VERSION` usa la de `Cargo.toml` con el sufijo `-local`.

## Instalación (sin root)

Cada versión de la página *Releases* trae siempre los mismos nombres de archivo y un `SHA256SUMS`
para comprobarlos, firmado con [cosign](https://github.com/sigstore/cosign) sin clave propia: la
firma (`SHA256SUMS.sigstore.json`) lleva un certificado de corta duración que Sigstore emite para la
CI de este repositorio en la rama principal y queda en su registro público de transparencia. Las
versiones publicadas antes de pasar a la organización `wallpaper-rs` no llevan firma. Con cosign 3
o posterior, junto a los archivos descargados:

```sh
cosign verify-blob SHA256SUMS --bundle SHA256SUMS.sigstore.json \
  --certificate-identity https://github.com/wallpaper-rs/wallpaper-rs/.github/workflows/ci.yml@refs/heads/master \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
sha256sum -c --ignore-missing SHA256SUMS
```

Descarga `wallpaper-rs-x86_64-linux.tar.gz` de la última y ejecuta:

```sh
tar xzf wallpaper-rs-x86_64-linux.tar.gz
cd wallpaper-rs-*-x86_64-linux
./install.sh
```

Instala todo en tu directorio personal: binarios en `~/.local/bin`, servicio de usuario de
systemd, activación por D-Bus, la app de escritorio (con su lanzador e icono), el plugin de fondo
y el icono de la bandeja de Plasma. Funciona también en distribuciones con
sistema de archivos inmutable. `install.sh` pide al bus de sesión que relea sus servicios, para que
la bandeja abra la app sin volver a iniciar sesión, también con dbus-broker, que no vigila los
servicios nuevos; si aun así el bus no conoce la app, la bandeja le pide que relea sus servicios y lo intenta
otra vez, y si tampoco así se abre, lo dice. Para
desinstalar: `./uninstall.sh` (añade `--purge` para borrar también la configuración).

Después, en Plasma: clic derecho en el escritorio → *Configurar escritorio y fondo de
pantalla* → tipo de fondo **wallpaper-rs**. Con escala fraccional y Qt anterior a 6.11, el plugin
pide más píxeles de los que tiene la pantalla; el daemon reduce el stream a los píxeles reales de la
pantalla, los del modo que anuncia el compositor Wayland, para no gastar GPU ni memoria de más (el
tamaño y la escala que le pasa el plugin no bastan, porque llevan la misma escala que la petición).

El icono de wallpaper-rs en la bandeja reúne lo básico: pausar o reanudar, silenciar, el volumen, el
perfil y una fila por pantalla con la miniatura y el nombre de su fondo y los botones *Siguiente*
(solo con una lista en marcha), *Elegir…* y *Favorito*; la miniatura de un paquete para adultos sale
difuminada hasta que haces clic en ella y, si el fondo destella, la fila lo avisa. También pregunta
qué permitir a una escena que lo pide. Todo lo demás está en la app: *Abrir wallpaper-rs* la abre y
*Ajustes…* la abre directamente en su página *Ajustes*, con el modo de ajuste, el color de fondo,
el filtro y la lista de cada pantalla o de todas a la vez, los ajustes de cada escena, los fotogramas
por segundo, las pausas y las transiciones. El filtro (botón *Filtro*, o `wprs-ctl filter`) colorea
cualquier fondo de una pantalla, imagen, vídeo, escena o página, sin tocar los archivos: uno de 15
estilos propios (cálido, frío, dorado, vivo, intenso, apagado, mate, desvaído, blanco y negro, noir,
sepia, atardecer, menta, rosa y noche) con su intensidad, más exposición, contraste, saturación,
calidez y tono. Va dentro de la pasada que dibuja el fondo en la pantalla, así que no añade otra, y
sin filtro no cuesta nada. Los perfiles guardan y ponen el filtro de cada pantalla junto con su fondo. Lo mismo hace `wprs-app --page settings` (o la acción
*Ajustes* del lanzador); `--page` acepta también `library`, `scenes`, `playlists`, `profiles`,
`providers`, `repos` y `account`. Si la app ya está abierta, pasa a esa página, salvo que el editor
de imágenes o el de escenas tenga cambios sin guardar: entonces solo se muestra la ventana y el
editor sigue abierto.

Requisitos: KDE Plasma 6.4 o posterior, PipeWire, GStreamer 1.24 o posterior (con los plugins base
y good y el de PipeWire, que lleva el sonido de los vídeos y de las escenas desde su versión 1.2;
sin él o con uno anterior, el sonido va a la salida por defecto; si aún no hay ninguna salida de sonido, el fondo
suena en cuanto aparece una) y un controlador Vulkan u OpenGL. `install.sh` avisa si no encuentra
GStreamer 1.24 o posterior, y el paquete RPM lo exige. El plugin de fondo usa los módulos QML de KPipeWire,
KWindowSystem y plasma-workspace; en Debian y Ubuntu vienen en `qml6-module-org-kde-pipewire`,
`qml6-module-org-kde-kwindowsystem` y `plasma-workspace`. La app de escritorio usa Qt 6 Quick,
Kirigami y el estilo de escritorio de KDE (`qml6-module-org-kde-kirigami` y
`qml6-module-org-kde-qqc2desktopstyle`).

## Instalación con Flatpak

La misma página *Releases* trae `wallpaper-rs.flatpak` (x86_64) y `wallpaper-rs-aarch64.flatpak`
(ARM de 64 bits), con la app de escritorio, el daemon, `wprs-ctl`, el plugin de fondo y el icono de
la bandeja de Plasma. El runtime (`org.kde.Platform` 6.11, con Qt 6, Kirigami, el estilo de
escritorio de KDE y GStreamer 1.26 con sus códecs; el Flatpak no se construye sobre uno con
GStreamer anterior a 1.24) se descarga de Flathub al instalarlo, así que el repositorio
`flathub` tiene que estar añadido; la primera orden lo añade si falta:

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user ./wallpaper-rs.flatpak
flatpak run --command=wprs-plasma io.github.wallpaper_rs.WallpaperRs | sh
```

Flatpak no puede instalar el plugin de fondo ni el icono de la bandeja de Plasma: la última orden
los instala con `kpackagetool6` desde los archivos del propio Flatpak y crea la carpeta en la que
Flatpak monta las ediciones compartidas, y deja `~/.local/share/wallpaper-rs/edits/`, que
Flatpak crea legible por todos, solo para tu usuario (sin `| sh` solo muestra las órdenes). Hay que repetirla después de cada actualización y reiniciar Plasma. El plugin arranca el
daemon por D-Bus cuando lo necesita; no hay servicio de systemd. En ARM se instala
`./wallpaper-rs-aarch64.flatpak`. No instales a la vez el Flatpak y el `.tar.gz`, porque los dos
registran los mismos servicios D-Bus: ejecuta antes `./uninstall.sh`.

La app sale en el menú de aplicaciones (`flatpak run io.github.wallpaper_rs.WallpaperRs` la abre
desde la terminal) y el icono de la bandeja la abre con *Abrir wallpaper-rs* y *Ajustes…*, como en
la instalación sin Flatpak. `wprs-ctl` también está dentro:

```sh
alias wprs-ctl='flatpak run --command=wprs-ctl io.github.wallpaper_rs.WallpaperRs'
wprs-ctl --help
```

Con el alias, `wprs-ctl` solo ve las carpetas que ve el Flatpak (Imágenes, Vídeos, Descargas y las
que añadas con `flatpak override`). Las rutas relativas valen cuando la carpeta actual es una de
ellas o está dentro; desde cualquier otra, `flatpak run` arranca en la carpeta personal y la ruta
relativa apunta a otro sitio, así que conviene usar la ruta completa: `wprs-ctl set
~/Imágenes/fondo.jpg` en lugar de `wprs-ctl set fondo.jpg`.

Permisos: la GPU, el socket de PipeWire, que publica los streams, lleva el sonido de los vídeos y de
las escenas y permite capturar el sonido solo para las escenas con permiso (no hace falta el de
PulseAudio), el socket de Wayland, para las ventanas de la app y, en el daemon, solo para leer la
resolución de cada pantalla (el daemon no abre ventanas), X11 solo en una sesión X11 y para la app,
los nombres D-Bus de la app y del daemon, el bloqueo de pantalla y la batería, el llavero del sistema (Secret
Service) para las contraseñas y claves de los proveedores, KWin para cargar el script del puntero
solo mientras una escena con permiso o un paralaje que sigue al puntero lo usa, las notificaciones
del escritorio, solo para preguntarte antes de dar a una escena el sonido o el puntero (dentro del
Flatpak el daemon no puede ver qué programa se lo pide, así que siempre pregunta así: la app, la
bandeja y `wprs-ctl input grant` avisan de que la pregunta está en la notificación, y la posición del puntero
solo la acepta del script de KWin), lectura de
Imágenes, Vídeos y Descargas, escritura solo en `~/.local/share/wallpaper-rs/edits/`, y red. La red
solo se usa para los proveedores de fondos, los repositorios y el servidor propio que añadas tú y
para el modelo de profundidad y ONNX Runtime cuando los pides. Los cambios de zona horaria llegan
sin permisos, por el fichero que Flatpak actualiza en el sandbox. Para usar fondos de otra carpeta:
`flatpak override --user --filesystem=/ruta:ro io.github.wallpaper_rs.WallpaperRs`.

Rutas, con `~/.var/app/io.github.wallpaper_rs.WallpaperRs` abreviado como `APP`: la configuración
vive en `APP/config/wallpaper-rs/`, la biblioteca, las escenas propias y los mapas de profundidad en
`APP/data/wallpaper-rs/` y la caché en `APP/cache/wallpaper-rs/`. Las ediciones de las imágenes que
el Flatpak solo puede leer (todas las de Imágenes, Vídeos y Descargas) no caben junto a la imagen y
van a `~/.local/share/wallpaper-rs/edits/`, la misma carpeta que usa un `wprs-ctl` de fuera del
Flatpak: el permiso de escritura sobre ella hace que Flatpak la monte también en
`APP/data/wallpaper-rs/edits/` en cuanto esa carpeta existe (la crea `wprs-plasma`), así que el daemon, la app y cualquier `wprs-ctl` ven las mismas
ediciones. Las que guardó una versión del Flatpak sin ese permiso se quedan tapadas en
`APP/data/wallpaper-rs/edits/` (solo dentro del Flatpak; fuera se ven) y se recuperan con
`mv -n APP/data/wallpaper-rs/edits/*.json ~/.local/share/wallpaper-rs/edits/`. Para desinstalar:
`flatpak run --command=wprs-plasma io.github.wallpaper_rs.WallpaperRs remove | sh` y
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
`wprsd --backend streams|layer-shell|x11` lo fuerza. Las pantallas se llaman como las anuncia el
compositor (`DP-1`, `HDMI-A-1`…; las muestran `swaymsg -t get_outputs`, `hyprctl monitors`,
`niri msg outputs` o `wlr-randr`) y esos nombres son los de `-o` y de la configuración. Las
pantallas que se conectan o se quitan con la sesión abierta se siguen solas, y la escala
fraccional se respeta. Un cambio de pantallas se aplica cuando lleva medio segundo estable, así que
una pantalla que el compositor anuncia dos veces, quita y vuelve a poner, o deja un instante sin
tamaño no rehace su superficie ni reinicia su vídeo. Si el compositor corta la conexión por un error
de protocolo, `wprsd` sale con estado 70 y el servicio de usuario lo arranca de nuevo.

Los vídeos se pausan cuando una ventana tapa la pantalla: el compositor deja de pedir fotogramas
al fondo, una ventana activa a pantalla completa o maximizada lo confirma y, en Hyprland, también
su IPC; en COSMIC, sus avisos de solapamiento, cuando las ventanas, junto con los paneles, cubren
todo el fondo. Una pantalla que vuelve a conectarse no se queda tapada por un aviso de la superficie
anterior, y si el compositor cierra la superficie del fondo con la pantalla conectada, `wprsd` la
vuelve a crear: tres veces por minuto como mucho, y si la cierra más veces, la crea otra vez en
cuanto pasa ese minuto. El Flatpak no sirve en estos escritorios: no dejan usar `wlr-layer-shell`
ni la información de ventanas a las aplicaciones aisladas.

## Sesiones X11

Con `DISPLAY` y sin sesión Wayland ni Plasma, `wprsd` pone en cada monitor de RandR una ventana de
escritorio debajo de todas las demás (si el gestor de ventanas no las coloca, una ventana
*override-redirect* que se mantiene al fondo) y dibuja en ella con la GPU. Los monitores se llaman
como en `xrandr --listmonitors` y se siguen al conectarlos o quitarlos. Con imágenes fijas también
pone el fondo de la ventana raíz (`_XROOTPMAP_ID` y `ESETROOT_PMAP_ID`) para los compositores y las
terminales con transparencia (y lo vuelve a pintar si la ventana raíz cambia de tamaño), y los
vídeos se pausan cuando una ventana del escritorio actual está a pantalla completa o maximizada en
ese monitor según EWMH, cuando varias ventanas en mosaico lo cubren entre todas o, sin compositor,
cuando el servidor avisa de que la ventana del fondo quedó tapada del todo. La ventana de escritorio
de otro programa, como la de los iconos de un gestor de archivos, queda siempre por encima del fondo
(y, si es opaca, la pantalla cuenta como tapada), y sin gestor de ventanas `wprsd` deja de disputar
el fondo de la pila a otro programa que mantiene allí su ventana después de bajar las suyas tres
veces en 10 s. Se arranca
con `wprsd &` en `~/.xinitrc`, antes del gestor de ventanas, o desde el arranque automático de
este; con el servicio de usuario,
`systemctl --user import-environment DISPLAY && systemctl --user restart wprsd`. Si el gestor de
ventanas arranca después que `wprsd` (o se cierra), las ventanas pasan solas a ser ventanas de
escritorio gestionadas (o *override-redirect*). Cuando una ventana deja de tapar el fondo, se dibuja
otra vez el último fotograma, también con el vídeo en pausa, sin reiniciarlo.

## Android

La misma página *Releases* trae un APK para Android 12 o posterior (64 bits) por arquitectura:
`wallpaper-rs-android-arm64-v8a.apk` para los teléfonos (unos 18 MB) y
`wallpaper-rs-android-x86_64.apk` para los emuladores y algunas tabletas (unos 19 MB), más
`wallpaper-rs-android.apk` con las dos (unos 31 MB) si no sabes cuál te toca. Antes un solo APK de
83 MB llevaba las dos arquitecturas, ONNX Runtime dentro y el código de Kotlin sin reducir.
Siempre se firman con la misma clave y la misma versión, así que cada versión se instala encima de
la anterior, también si cambias de uno a otro, y conserva el fondo elegido. Ábrelo en el teléfono
(Android pedirá permitir la instalación desde esa app) o instálalo desde un ordenador con
`adb install -r wallpaper-rs-android-arm64-v8a.apk`.

Para ponerlo de fondo, abre la app **wallpaper-rs**. La pestaña *Biblioteca* muestra las fotos y
los vídeos añadidos con *Añadir*, que ofrece tres caminos: *Fotos y vídeos* abre el selector de fotos
de Android, *Elegir archivos* abre el selector de documentos para escoger varias imágenes y vídeos de
cualquier carpeta, de la tarjeta o de una nube instalada, e *Importar una carpeta* añade las imágenes
y los vídeos que hay dentro de una carpeta y de sus subcarpetas (5000 como mucho) mostrando cuántos
lleva de cuántos. Todo se copia al almacenamiento privado de la app, así que no hace falta ningún
permiso y la app no usa la red salvo para descargar el modelo de profundidad cuando lo pides, para
tu servidor propio y los repositorios que añades y, solo mientras escribes la dirección de un
servidor, para buscar en la red local los que se anuncian. Un mismo archivo no se copia dos veces, y los que no se pueden abrir
se cuentan en un solo aviso. Al tocar uno se abre su página, con una vista previa en vivo con la
forma de la pantalla, sus medidas y duración, y los botones *Usar como fondo*, favorito, *Añadir a
una lista* y *Quitar*.

Una pulsación larga en la biblioteca, o en una lista abierta, empieza a seleccionar: cada toque marca
o desmarca un elemento, la barra de arriba muestra cuántos hay con *Seleccionar todo* y *Cancelar*, y
sus acciones sirven para todos a la vez: añadirlos a una lista, marcarlos o desmarcarlos como
favoritos, elegir su ajuste y quitarlos de la biblioteca con una sola confirmación (en una lista,
sacarlos de ella). El gesto de volver sale de la selección, que se conserva al girar el teléfono.

En la página de una imagen, *Ajuste* elige cómo se coloca esa imagen (rellenar, imagen completa,
estirar, centrar o mosaico, o como en *Ajustes*), y arrastrar o pellizcar la vista previa elige la
parte que se ve en el teléfono; *Restablecer el encuadre* lo quita y vuelve a la imagen entera o al
recorte hecho en el ordenador. El ajuste es un campo de la misma edición no destructiva que usa el
escritorio (`wprs-ctl edit fit`), y el encuadre, un recorte con la forma de la pantalla del teléfono
que la edición guarda como el de una pantalla más, `Android` (`wprs-ctl edit crop -o Android`): se
sincroniza con el ordenador sin recortar sus pantallas, y se hace dentro del recorte de la imagen
entera si se eligió en el ordenador. Se combina con el paralaje; el paso de páginas del lanzador
recorre la imagen, o el recorte del ordenador si es más ancho que la pantalla, pero no mueve un
encuadre hecho en el teléfono, que ya tiene su forma. La vista previa usa la forma de la pantalla en
su orientación natural, también con el teléfono tumbado, donde se pone al lado de los controles.
*Usar como fondo* pregunta si va en la pantalla de inicio, en la de bloqueo o en las dos (desde
Android 14; antes, las dos muestran lo mismo), y cada pantalla guarda su propio fondo o su propia
lista, igual que una pantalla del escritorio (`[outputs.lock]` en la configuración). Si la app ya
es el fondo de esa pantalla, el cambio entra con un fundido sin pasar por la vista previa del
sistema; si no, o si un mismo fondo de la app cubre las dos pantallas y se elige solo una, abre la
vista previa del sistema para ponerlo allí. También se llega desde los fondos animados del sistema. Se dibuja con la GPU (Vulkan, u
OpenGL ES si Vulkan falla) con el mismo renderizador que el escritorio, y se recupera sola si el
sistema le quita la GPU o la superficie. La edición hecha en el escritorio se aplica igual: el giro,
el enderezado, el color y las máscaras, también las de profundidad, que seleccionan sobre el mapa de
profundidad de la foto después del mismo giro y la misma orientación que el escritorio. Admite JPEG, PNG, WebP, GIF y BMP y, con el
decodificador del propio Android (`AImageDecoder`, sin librerías añadidas al APK), las fotos HEIC y
AVIF de las cámaras. El escritorio no lee HEIC, así que no las añade a su biblioteca al recorrer
sus carpetas ni al recibir los archivos privados del teléfono.

La pestaña *Listas* crea listas de fondos que cambian solos cada 15 o 30 minutos, cada hora, cada 6
horas, cada día o nunca, en orden o al azar y, si se quiere, saltando los vídeos con batería. Como en
el escritorio, una lista puede cambiar solo ciertos días de la semana y a ciertas horas (`days` y
`at` de sus reglas), seguir donde se quedó o empezar de nuevo (`resume`), mostrar el mismo fondo en
la pantalla de inicio y en la de bloqueo (`synced`) y tener su propio fundido (`transition_ms`),
cambia de nombre con el lápiz de su página y sus fondos se reordenan
arrastrándolos por el asa de la derecha. Su *Búsqueda inteligente* la convierte en una lista
inteligente con la misma sintaxis que `wprs-ctl search` (por ejemplo `kind:image tag:mar`): muestra
cuántos fondos encuentra mientras se escribe, se guarda con *Guardar la búsqueda* y *Hacerla manual*
la vuelve a dejar como estaba. La biblioteca también tiene un campo de búsqueda con esa sintaxis,
que se suma al filtro elegido. El cambio se decide cuando el fondo vuelve a verse, sin alarmas ni
despertar el teléfono, también con días y horas. La biblioteca, las listas y sus reglas usan el
mismo modelo y la misma lógica de rotación que el escritorio.

La pestaña *Ajustes* elige cómo se ajustan las imágenes que no tienen ajuste propio y los vídeos
(rellenar, imagen completa, estirar o centrar), el fundido entre fondos, el sonido de los vídeos,
los fotogramas por segundo con batería (30, 15 o 10) y con el cargador (60 o 30), cuándo cambian los
colores del sistema y los gráficos (automático, Vulkan u OpenGL ES). Debajo, el diagnóstico muestra
la GPU, los fotogramas dibujados y por segundo, el límite que se aplica en ese momento, el estado de
carga, ahorro de energía y temperatura y, con una escena, la clase del teléfono (y si se midió en su
GPU), el nivel de calidad y por qué bajó. Los avisos que vienen del motor (los fallos de un
paquete, de una propiedad, de la profundidad o de la sincronización) llevan un identificador de
mensaje y se muestran en el idioma del teléfono con las mismas traducciones que el resto de la app.
*Licencias* muestra los textos de ONNX Runtime, de los componentes que incluye y del modelo
de profundidad. El diagnóstico dice también si la clase del teléfono se midió con marcas de
tiempo de la GPU, que cuentan solo el trabajo de la GPU, o esperando a que termine, en los
teléfonos que no las tienen; el escritorio mide igual.

Los vídeos (MP4, MOV, 3GP, WebM o MKV) se repiten en bucle. Con el ajuste *Rellenar* o *Estirar*
los decodifica el hardware del teléfono directamente sobre el fondo, sin pasar por la GPU ni copiar
fotogramas; con *Rellenar* piden a Android que llenen la pantalla recortando los bordes, un recorte
que hace el compositor del sistema, así que un teléfono que no lo respete los mostrará estirados.
Con *Imagen completa* o *Centrar* se dibujan con la GPU, como los vídeos de las escenas (el hardware
los decodifica y cada fotograma se copia a la GPU, como mucho a los fotogramas por segundo
permitidos), con franjas del color de fondo alrededor. Se pausan en cuanto el fondo deja de verse y
sueltan el decodificador si siguen ocultos más de 30 segundos; al volver siguen por donde iban. Si
inicio y bloqueo muestran el mismo vídeo, comparten un solo reproductor. Van sin sonido salvo con
*Reproducir el sonido de los vídeos*. El fundido entre fondos sirve entre todos los tipos: de una
imagen, una escena o un vídeo a otro de cualquier tipo, el que sale y el que entra se mezclan; un
vídeo entra o sale por la GPU durante el fundido y después vuelve a la ruta directa sin cortarse ni
volver a empezar. Al añadir un vídeo, la app comprueba que el teléfono tiene un decodificador para
su pista de vídeo y saca su primer fotograma, como el escritorio comprueba que GStreamer puede
abrirlo; si no puede, no lo añade y lo cuenta en el aviso de los archivos que no se pudieron abrir.
Los vídeos no se convierten al añadirlos: se reproducen tal cual.

En la página de una imagen, *Moverse con el teléfono* activa el paralaje. Sigue el giro del teléfono
con el sensor de rotación que no usa la brújula o, si no lo hay, con el normal: girarlo hacia un
lado mueve la imagen en horizontal y inclinarlo hacia delante o hacia atrás, en vertical, también
con el teléfono de pie. Sin giroscopio sigue la inclinación con el acelerómetro, y si el teléfono no
tiene ningún sensor o no da datos, el paso de páginas del lanzador. El giro se filtra y se recentra
solo en unos segundos, con el mismo muelle y el mismo sombreador que el escritorio: con un mapa de
profundidad lo cercano se mueve más que el fondo, y sin él la imagen entera se desplaza.
*Intensidad* y *Enfoque* (la profundidad que se queda quieta) son los mismos valores que en el
escritorio, pero en el teléfono la intensidad se mide sobre la parte que se ve y vale el triple,
porque la pantalla es pequeña y se mira de cerca: con los valores por defecto lo cercano se desplaza
hasta un 4 % del ancho de la pantalla y el fondo hasta un 2 % hacia el otro lado, y la imagen se
amplía lo justo para no enseñar los bordes. La página muestra si la imagen aún no tiene mapa de
profundidad, si se está calculando o si ya está listo, y avisa cuando el movimiento está en pausa
por el ahorro de energía, por poca batería o por calor; con una batería normal, con o sin cargador,
nunca se para. El sensor solo se enciende mientras se ve un fondo con paralaje, a los fotogramas por
segundo permitidos (60 como mucho), y se apaga al taparlo. Con el teléfono quieto no se dibuja
ningún fotograma, y con batería el sombreador da como mucho 8 pasos de profundidad.

*Calcular un mapa de profundidad* usa el mismo modelo que el escritorio, Depth Anything V2 Small,
con ONNX Runtime 1.31.0, que ya no va dentro del APK. La primera vez pide permiso para descargar los
dos y muestra su tamaño (154,2 MB en total: 99,1 MB del modelo y 55,1 MB del paquete de ONNX Runtime
para Android), sus licencias (Apache-2.0 y MIT) y los servidores (github.com y repo1.maven.org). Las
descargas llevan el User-Agent genérico del escritorio, se reanudan si se cortan y se comprueban
con su SHA-256 fijado; del paquete de ONNX Runtime solo se guarda la biblioteca de la arquitectura
del teléfono (unos 34 MB en arm64-v8a), comprobada también con su propio SHA-256, de solo lectura
y con el texto de su licencia al lado, y el paquete se borra. El mapa se calcula en segundo plano
en el propio teléfono, con la telemetría de ONNX Runtime apagada, y la foto no sale del teléfono;
*Cancelar* detiene la descarga o el cálculo en cuanto se pulsa, también a mitad del modelo. Con un
mapa, la página muestra los mismos ajustes que el escritorio (`wprs-ctl edit depth`): nivel
cercano y lejano, curva, suavizado y cuánto crecen los bordes cercanos, que cambian el paralaje y
las máscaras de profundidad al soltarlos, el mapa ajustado en gris o en *Falso color: rojo cerca y
azul lejos*, como en la app, y *Quitar el mapa de profundidad* borra el mapa calculado y el
importado. *Ejecutar el modelo en* elige, como en la app, el dispositivo más rápido o el procesador
(en el teléfono el más rápido es hoy el procesador, porque su ONNX Runtime no trae aceleradores), y
la página avisa, como el escritorio, si la imagen cambió después de editarla. *Usar un archivo de mapa de profundidad*
importa un mapa hecho en el escritorio, el PNG de 16 bits que guarda junto a la imagen (por ejemplo
`foto.jpg.depth.png`), si tiene la misma proporción. En *Ajustes*, *Permitir descargar el modelo de
profundidad* (`models.allow_download`) bloquea la descarga, y ahí mismo se puede borrar el modelo.

Los filtros *Escenas* y *Páginas web* de la biblioteca tienen *Añadir las escenas de ejemplo*, que
instala las cinco escenas y la página web *Luces* que trae el APK (los mismos ejemplos que el
escritorio), y *Abrir un paquete*, que importa un `.wpak` con el selector de documentos de Android.
Se admiten los cuatro tipos de paquete que son un fondo, como en el escritorio: los de imagen se
muestran como una foto más (con su ajuste, su encuadre, su edición y su paralaje), los de vídeo como
un vídeo más, los de escena con el motor de escenas y los de página web con el WebView del sistema
(ver «Fondos web»); los de preajustes y de recursos se rechazan con un aviso, igual que en el
escritorio. La imagen o el vídeo del paquete se extrae al importarlo, por partes y sin cargarlo
entero en memoria, a los archivos de la app y se borra con el paquete. Las escenas se dibujan con el mismo motor que el
escritorio: capas, efectos, partículas (en la GPU si admite sombreadores de cálculo y, si no, en la
CPU con menos partículas), textos, líneas de tiempo y propiedades. La página de una escena muestra
sus propiedades y sus preajustes, que se guardan en la misma configuración que el escritorio; los
preajustes propios se guardan, se renombran y se borran ahí, y se exportan e importan como archivo,
los botones de la escena funcionan y los textos secretos quedan sellados por el almacén de claves de
Android. Las propiedades de archivo y de carpeta se eligen con el selector de documentos de
Android: el archivo elegido (una imagen o, si la propiedad los admite, un vídeo) se copia a los
archivos de la app, y de una carpeta se copian las imágenes y los vídeos que admite, con sus
subcarpetas hasta la profundidad que pide y como mucho los archivos que pide. La app guarda el
permiso de lectura de la carpeta y, cada vez que se abre, copia otra vez lo que cambió en ella;
*Quitar* borra la copia y devuelve el permiso. Como en el escritorio, la escena muestra el archivo
de la carpeta que toca, en orden o al azar y cada el tiempo que pide, y solo cambia mientras se ve.
Los ajustes de reproducción de los paquetes de vídeo no se usan en el teléfono, y la página del
paquete lo avisa, igual que de todo lo que el teléfono omite: el color alrededor de la imagen y la
extensión sobre varias pantallas, el límite de fotogramas, el inicio en silencio y la reproducción
única de vídeos y páginas, los atajos, los sonidos nuevos, lo que no tiene sentido en el teléfono
(los sonidos de los scripts) y la iluminación RGB. Los scripts de escena guardan datos, crean formas,
mueven la cámara, cambian los padres de las capas y listan la carpeta elegida igual que en el
escritorio. El filtro de color de la sección `[default]` de `config.toml` y el volteo de imágenes y
vídeos se aplican también aquí; un vídeo con filtro o volteado pasa por la gráfica en vez de
reproducirse directo en la pantalla. Un vídeo HDR se ve como lo decodifica Android: al reproducirse
directo, el sistema adapta sus tonos. Si un paquete pide sombreadores de cálculo y la gráfica del teléfono no los tiene, su página lo
avisa. El color de acento del fondo (o, si no propone uno, el dominante de su vista previa) es el
color que Android da al sistema. Un paquete de más de 1 GiB descomprimido se rechaza, y los
preajustes publicados aparte y las dependencias entre paquetes aún no funcionan en el teléfono. Un paquete hecho solo para
el escritorio se rechaza con ese aviso, traducido. El nivel de calidad sale de la
clase del teléfono (básica con poca memoria o sin sombreadores de cálculo, alta si el fabricante
declara una clase de rendimiento de Android 13 o posterior, media en el resto, ajustada con la misma
medida corta que en el escritorio y guardada en los archivos de la app) y de la batería: con
batería baja un nivel, y con calor baja más; si los fotogramas tardan demasiado en la GPU baja otro,
y solo vuelve a subir con el cargador puesto y el teléfono frío. El motor de la escena sigue vivo
cuando el sistema cambia la superficie del fondo, así que no se vuelve a cargar ni compilar. Las capas con
paralaje de una escena siguen la inclinación del teléfono con los mismos sensores que el paralaje de
las fotos, aunque la escena no lo pida con `gyro`, y sin sensores, el paso de páginas del lanzador;
con el ahorro de energía se quedan quietas. Una escena demasiado pesada para el teléfono se queda
quieta en su fotograma de muestra. Cada escena dibuja a sus propios fotogramas por segundo, como
mucho a los de los ajustes, solo mientras algo cambia y nunca mientras no se ve. Los vídeos dentro
de una escena los decodifica el hardware y se copian a la GPU fotograma a fotograma, sin sonido, y
se pausan si la escena se queda quieta. Los scripts de las escenas se ejecutan en el teléfono como en
el escritorio, en un hilo propio con los mismos límites (QuickJS sin archivos ni red, 32 MiB de
memoria y 4 ms por fotograma); si uno falla tres veces seguidas se detiene y la escena sigue sin él,
y el diagnóstico de *Ajustes* lo dice. Sus sonidos no suenan en el teléfono, así que su volumen no
aparece entre las propiedades. El diagnóstico también dice si los efectos de los paquetes se
cronometran con marcas de tiempo de la GPU o esperando a que termine, y al cerrarse el fondo el
teléfono termina las compilaciones de sombreadores y limpia sus marcas de prueba, para que un cierre
normal no cuente como un efecto que colgó la GPU.

Las miniaturas de los paquetes para adultos salen difuminadas y su página no los muestra hasta
pulsar *Mostrar igualmente*, salvo que *Ajustes* muestre el contenido para adultos sin difuminar.
*Ocultar el contenido para adultos* (`library.hide_mature`, desactivado por defecto), justo encima,
los saca de la biblioteca y de sus búsquedas, igual que en la app de escritorio y en `wprs-ctl`, salvo
que la búsqueda pida `content:` ella misma. Un paquete fotosensible no se previsualiza ni se puede usar como fondo
hasta aceptar el aviso de que destella o muestra patrones rápidos que pueden provocar crisis a
personas con epilepsia fotosensible; las listas lo saltan mientras tanto. Cada decisión se recuerda
por paquete. La biblioteca y la página de un paquete muestran su título y su descripción en el
idioma del teléfono, y la página avisa de cada parte que no funciona en él, de sus sonidos a sus
ajustes de vídeo y las funciones nuevas que omite.

*Reaccionar al sonido*, *Reaccionar al tacto* y *Reaccionar a los toques* solo aparecen en las
escenas y las páginas web que los usan y se activan paquete a paquete; empiezan apagados. El sonido se lee con el visualizador de Android, que da un espectro de
poca resolución de lo que suena en el teléfono; Android pide el permiso del micrófono la primera vez
que se activa y muestra su indicador mientras escucha. Solo escucha mientras se ve una escena con
ese permiso, y solo calcula bandas en memoria. Tras dos segundos de silencio deja de leerlo hasta que
algo empieza a sonar; si un reproductor sigue activo en silencio, lo mira cada dos segundos. El tacto
es la posición del dedo sobre el fondo en la pantalla de inicio, que la escena sigue como el puntero,
y los toques son los clics del teléfono: un toque sobre una capa `solid` arranca sus líneas de tiempo
`click:` y llega a sus scripts. Solo cuentan los toques reales sobre la pantalla de inicio (la de
bloqueo no da ninguno) y sobre la vista previa de la página de la escena, y solo en las escenas
con ese permiso; sin él, tocar la vista previa no le llega a la escena.

Solo se dibuja cuando cambia algo y nunca mientras el fondo no se ve: con una imagen fija no hay
ningún fotograma más. El fundido y el desplazamiento al pasar de página en el lanzador van como
mucho a los fotogramas por segundo elegidos (30 con batería y 60 con el cargador si no se cambian, y
30 en la vista previa), se detienen con el ahorro de energía o con poca batería y bajan cuando el
teléfono se calienta. Los vídeos se pausan con el ahorro de energía, con poca batería o con el
teléfono muy caliente. Los colores del sistema (Material You) salen de la imagen o del primer
fotograma del vídeo y se calculan una sola vez al añadirlos. *Colores del sistema* decide si Android
cambia de tema al elegir un fondo (lo normal), también en cada cambio de una lista o nunca, y Android
lo aplica la próxima vez que se apaga y enciende la pantalla.

## Uso desde la terminal

```sh
wprs-ctl set ~/Imágenes/fondo.jpg               # todas las pantallas
wprs-ctl set ~/Vídeos/mar.mp4 -o DP-1 -f fit    # una pantalla, modo de ajuste
wprs-ctl fit center                             # solo el modo de ajuste, mismo fondo
wprs-ctl fit tile -o DP-1                       # el modo de ajuste de una pantalla
wprs-ctl background '#203040'                   # color alrededor del fondo
wprs-ctl filter warm --strength 0.6 -o DP-1      # filtro de color de una pantalla (none lo quita)
wprs-ctl filter --list                          # los filtros que hay
wprs-ctl pause | resume | toggle                # pausar los fondos animados
wprs-ctl volume 40 | mute | unmute              # sonido de los vídeos y las escenas
wprs-ctl config                                 # ver los ajustes
wprs-ctl config transition.duration_ms 1000     # cambiar un ajuste
wprs-ctl state                                  # estado en JSON
wprs-ctl reload                                 # releer la configuración
```

Modos de ajuste: `fill`, `fit`, `stretch`, `center`, `tile` y `span` (una imagen sobre todas las
pantallas, con su tamaño físico y los marcos entre ellas). Las pantallas que muestran la misma
imagen con la misma edición, como las de `span` o las que tienen su propio recorte, comparten una
sola copia en la GPU, y los JPEG grandes se decodifican directamente a un tamaño reducido. Con una
escena pasa lo mismo: cada imagen del paquete se sube una vez y la usan todas las pantallas que la
muestran, la copia decodificada sale de la memoria en cuanto está en la GPU, los sombreadores se
compilan al cargar la escena y un fotograma estable no crea recursos ni reserva memoria propia,
tampoco al evaluar la escena, que reutiliza las listas del fotograma anterior.
Ajustes: transición (`fade` o `none` y su duración), fotogramas por segundo y cuándo pausar
(ventana encima, batería,
bloqueo). Un ajuste desconocido termina con error y muestra la lista de ajustes válidos.

`playback.max_fps` vale `display` por defecto (*Igual que la pantalla* en la app):
con corriente cada pantalla se dibuja como mucho a su frecuencia de refresco (la que da el
compositor al presentar cada superficie o la del modo de `wl_output`, la de RandR en X11 o, en
Plasma, la que da KWin) y con batería a 30 fotogramas por
segundo; un número fija el tope (`wprs-ctl config playback.max_fps 60`). Salvo en los vídeos, la
GPU pone su propio límite: el render por software se queda en 24 (15 con batería), una GPU
integrada en 60 (30 con batería) salvo escenas ligeras con corriente, y una dedicada sigue a la
pantalla hasta 240 (60 con batería). Los vídeos se quedan en 30 con `display` y render por
software y nunca pasan de sus propios fotogramas, nada se dibuja si nada cambia y `wprs-ctl state`
muestra la frecuencia (`refresh`) y el tope (`fps`) de cada pantalla.

Las escenas calculan antes de dibujar cuánto cuesta cada fotograma: cuentan sus efectos, las capas
que se dibujan aparte y las mezclas que copian el fondo, cada una por la parte de la pantalla que
ocupa, y eligen el nivel de calidad más alto que cabe en la GPU. Las GPU integradas antiguas (Intel
HD y UHD, las APU de AMD con Vega y similares) y los equipos con menos de 4 GiB de memoria tienen un
nivel intermedio propio. La primera vez que se usa una tarjeta gráfica con un driver, un render
corto de unos 300 ms mide cuánto tarda y ajusta su clase, que se guarda en
`~/.local/state/wallpaper-rs/gpu-tier.json` y se vuelve a medir solo si cambian la tarjeta o el
driver. La medida mueve la clase un paso como mucho, y si la baja (un arranque con el equipo ocupado
puede hacerlo) se repite en los arranques siguientes, hasta tres veces, y cuenta la mejor. Mientras
la escena se mueve, el tiempo que pasa cada fotograma en la GPU, partículas incluidas, guía la
calidad: si durante 2 s pasa de la mitad del intervalo entre fotogramas (de una cuarta parte con
batería), baja un nivel, y solo vuelve a subir si, con lo que costaba el nivel de arriba, durante
15 s no llegaría a la mitad del intervalo de ese nivel, así que una escena no salta entre dos
niveles cuando el de abajo además dibuja menos fotogramas por segundo. Cuánto más caro se toma el
nivel de arriba nunca pasa de una vez y media lo que estima el modelo de costes, para que una carga breve
justo antes de bajar no deje la escena abajo, y si solo eso la retiene, a los 60 s prueba otra vez
el nivel de arriba; cada vez que tiene que volver a bajar en menos de 30 s, las esperas se duplican
(hasta cuatro veces) y se acortan otra vez cuando una subida se mantiene. Nunca sube con batería ni
por encima de lo que permiten la clase, la batería y la temperatura. Sin marcas de tiempo en la GPU
mide uno de cada quince fotogramas esperando a la GPU. `wprs-ctl state` (`tier`,
`tier_measured`, y `quality` y `quality_reason` en cada pantalla), la app y la bandeja muestran la
clase medida, el nivel y por qué bajó. Con batería o con el perfil de ahorro de energía de power-profiles-daemon
las escenas bajan de nivel y de fotogramas; si un sensor del sistema dice que la GPU o la CPU están
cerca de su temperatura crítica, bajan más hasta que se enfrían. Cuando el escritorio pide reducir
el movimiento (`reduced-motion` del portal de ajustes), las escenas se quedan quietas en su
fotograma de muestra y solo se redibujan una vez por minuto para que los relojes sigan en hora,
salvo las escenas a las que dejes seguir moviéndose (*Seguir en movimiento cuando el escritorio pide
reducir el movimiento* en los detalles del paquete en la app, el mismo interruptor en su página del
teléfono, `wprs-ctl lib confirm motion ELEMENTO` y `wprs-ctl lib unconfirm motion ELEMENTO` para
volver atrás), pensado para escenas tranquilas: no se ofrece para los paquetes fotosensibles. Los
efectos lentos, como las nubes o las ondas, declaran a cuántos fotogramas por segundo se ven bien y
una escena que solo se mueve por ellos no pasa de ahí. Cada fotograma redibuja solo la parte de la
pantalla que cambió y, si nada cambió, no dibuja nada; las partículas quietas y las escenas
detenidas en un fotograma no cuentan como cambio. Con render por software cada escena se dibuja a
como mucho un megapíxel por núcleo y se escala a la pantalla, la espera a la GPU llega hasta 60 s
con pocos núcleos (tras un fotograma con efectos de paquetes, también al componerlo, la espera sigue
siendo de 10 s para detectar cuelgues) y, si un
fotograma tarda más de un octavo de esa espera, la resolución baja por pasos y vuelve a subir cuando
sobra tiempo, así que un fotograma pesado nunca se toma por una GPU colgada. Los desenfoques y las deformaciones ya no
oscurecen los bordes derecho e inferior, y el desenfoque de movimiento puede seguir la velocidad de
su capa (`follow`).

Para quien es sensible a los destellos, `playback.flash_guard` (*Frenar o detener los fondos que
destellan más de tres veces por segundo* en la página *Ajustes* de la app, que el botón *Ajustes…*
de la bandeja abre, un interruptor propio en la bandeja, `wprs-ctl config playback.flash_guard true` y
un interruptor en los ajustes del teléfono) mide el brillo de cada fotograma de las escenas y, si una destella más de tres veces por
segundo, la reproduce más despacio o la detiene; `wprs-ctl state` lo indica en `flashes`. Está apagado por
defecto porque cuesta una pequeña lectura de la GPU por fotograma.

Los streams llevan cada fotograma en memoria de la GPU (DMA-BUF) cuando el controlador Vulkan
puede exportarla y el consumidor (KPipeWire en Plasma) importarla: el daemon dibuja directamente en
los búferes del stream, sin leer el fotograma de vuelta ni copiarlo con la CPU, también con batería.
Donde la GPU no puede dibujar en el formato que pide el consumidor, como las imágenes lineales en
las GPU de NVIDIA, dibuja en una textura propia de cada pantalla y la copia en la GPU al búfer del
stream.
El daemon compone y envía solo el rectángulo que cambió: en memoria de la GPU dibuja en cada búfer
lo que cambió desde la última vez que lo usó, en memoria compartida lee de la GPU y copia solo esas
filas, y los dos tipos de búfer llevan `SPA_META_VideoDamage` con lo que cambió desde el búfer
anterior para los consumidores que lo leen. Un reloj pequeño sobre una imagen 4K copia así unos 55 KiB por
fotograma en vez de 32 MiB. Las superficies de Wayland y X11 y el teléfono siguen componiendo el
fotograma entero, porque la API gráfica no dice qué contiene cada búfer de la pantalla.
Si algo falta (OpenGL, render por software, un consumidor sin DMA-BUF), el stream usa memoria
compartida sin intervención; `WPRS_DMABUF=0` en el entorno del daemon la fuerza y `WPRS_DMABUF=1`
pide memoria de la GPU también con render por software. `wprs-ctl state` muestra el tipo de búfer
de cada pantalla (`buffers`: `dmabuf` o `memfd`). Con `WPRS_SYNCOBJ=1` en el entorno del daemon,
los búferes de la GPU ofrecen además sincronización explícita (syncobj) a los consumidores y
versiones de PipeWire que la admiten; es experimental y está apagada por defecto, así que sin esa
variable los búferes se sincronizan solo de forma implícita.

Cuando el decodificador de vídeo por hardware entrega los fotogramas en memoria de la GPU (DMA-BUF en
NV12, como VA-API en las GPU de Intel y AMD) y el controlador Vulkan puede importarlos con su
modificador, el daemon los lee en su sitio: convierte el color, gira el vídeo si lo pide y lo reduce
al tamaño de la pantalla en la GPU, sin pasar el fotograma por la memoria del sistema (cero copias
con la CPU por fotograma, frente a tres en la ruta de copia: bajar el fotograma, convertirlo a RGBA y
subirlo; con un vídeo 4K a 30 fps decodificado por VA-API en una iGPU de AMD, unos 0,3 ms de CPU
por fotograma en vez de unos 16 ms). Si el decodificador es por software, si el formato o el modificador no se pueden importar
o si la importación falla, el vídeo vuelve solo a la ruta de copia. En las GPU de NVIDIA, GStreamer
1.24 no entrega DMA-BUF (NVDEC da memoria CUDA u OpenGL y el controlador VA-API sobre NVDEC solo
memoria del sistema), así que allí los vídeos siguen copiándose, aunque la importación funciona; con
lavapipe y OpenGL no hay importación. `wprs-ctl state` muestra la ruta elegida en `decode`
(`zero-copy` o `copy`) y `WPRS_ZERO_COPY=0` en el entorno del daemon fuerza la copia. Pausar un vídeo
lo detiene donde está, sin volver a decodificar desde el fotograma clave anterior: en un vídeo 4K
con fotogramas clave cada 10 s, una pausa pasó de unos 2,3 s de CPU a menos de 20 ms.

La configuración vive en `~/.config/wallpaper-rs/config.toml` y solo la puede leer su usuario.
Si se edita a mano, los valores fuera de rango se corrigen al leerla. Si tiene un error de
sintaxis, el daemon arranca con los valores por defecto y muestra el error en `wprs-ctl state`
(`config_error`); el archivo roto se copia a `config.toml.broken` antes de guardar otro. Al
guardar un cambio (desde la app, la bandeja, `wprs-ctl config` o el teléfono) el archivo conserva
sus comentarios, sus líneas en blanco, el orden de las claves (también dentro de cada proveedor) y
cómo están escritos los valores: solo se reescribe lo que cambió.

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
wprs-ctl input grant org.example.dawn audio     # dejar que la escena que se ve siga el sonido (-o elige pantalla)
wprs-ctl input status                           # permisos y si se captura el sonido o el puntero
wprs-ctl input clicks on -o DP-1                # dejar que los clics de esa pantalla lleguen a su fondo
```

Los repositorios aguantan los fallos. Las búsquedas leen todos los repositorios y los paquetes
instalados de una vez, así que nunca mezclan un catálogo con otra lista de instalados; dos
actualizaciones del mismo repositorio desde procesos distintos nunca lo devuelven a un estado
anterior, y una instalación comprueba otra vez, al terminar la descarga y antes de pasar a la
biblioteca, que el repositorio sigue ahí y que ni la versión ni su clave se revocaron. Si la copia
guardada de un repositorio se daña (falta su `state.json`, por ejemplo), `wprs-ctl remote ls` y la
app lo muestran como roto sin afectar a los demás: `wprs-ctl remote refresh` lo descarga otra vez
de su dirección y, si esta se perdió, se repara añadiéndolo de nuevo con su URL o se quita. Los
espejos `file://` de un repositorio firmado solo se usan cuando el propio repositorio se añadió
desde una carpeta o una URL `file://`. Cancelar una descarga la para en una décima de segundo
aunque el servidor no envíe nada (antes esperaba hasta 30 s), también en las copias desde
`file://`. Las vistas previas de las versiones anteriores se descargan una sola vez y se borran
con su repositorio. Un proveedor corta una descarga en cuanto no cabe en `cache_mb` (por su
`Content-Length` o al llegar a ese tamaño), su limpieza de la caché resuelve los enlaces simbólicos
y nunca borra fuera de su carpeta, y quitarlo sin `--purge` borra también su estado de
sincronización, así que al añadirlo de nuevo sincroniza enseguida. Si el llavero está bloqueado o
no responde cuando se añade un proveedor con el nombre de uno anterior, el secreto viejo nunca se
usa: `wprs-ctl provider ls` y la app lo avisan, y se borra solo en cuanto el llavero se desbloquea
o vuelve.

La vista previa animada de un paquete (`previews.animated`) se mueve en los detalles de la app,
en la miniatura de cada pantalla de la app y de la bandeja, y en la página del teléfono de un
paquete que este no puede reproducir; solo mientras se ve, en el escritorio con el equipo
enchufado y, si el paquete es para adultos o fotosensible, después de permitirlo. Al explorar los
repositorios en la app también se mueve la vista previa animada del paquete que tiene el puntero o
el foco: se descarga entonces y solo con el equipo enchufado, dentro de su tamaño y su hash firmados
(nunca si pasa de los 8 MiB que admite un paquete), se guarda con las demás vistas previas y se
borra con su repositorio; nunca se mueve la de un paquete fotosensible ni la que sigue difuminada. `wprs-ctl pkg preview --animated PAQUETE` da la misma
ruta.

La biblioteca de la app pide de nuevo solo los elementos que nombra cada cambio (con `uid:`, que
también acepta `wprs-ctl lib search 'uid:<uid>,<uid>'` con uids completos), y al cambiar o repetir una búsqueda
conserva la selección y la posición de los elementos que siguen en ella.

Las consultas aceptan `--json` e imprimen la respuesta del daemon tal cual; sin él muestran
tablas para leer, no para scripts. Lo que lanza un trabajo imprime su número y termina; con
`--wait` muestra el progreso y el resultado, y Ctrl-C deja de esperar sin parar el trabajo
(`wprs-ctl cancel <n>` lo para). Un trabajo que falla siempre dice por qué: si el error no trae
descripción, el daemon envía `failed without giving a reason` con el nombre del error, la app lo
muestra en tu idioma y el teléfono da también un motivo traducido; en una pregunta (como instalar desde un repositorio caducado),
Ctrl-C la deja sin respuesta y no cambia nada. Las contraseñas y claves de los proveedores se piden
sin eco o se leen de la primera línea de la entrada con `--secret-stdin`; nunca van en la orden.
`wprs-ctl input grant` da el permiso al paquete que se ve con ese id; si varias pantallas muestran
paquetes distintos con el mismo id, `-o` elige la pantalla, y si el daemon pregunta en una
notificación (en el Flatpak, siempre), lo avisa y termina bien: la escena recibe la entrada cuando
la permites allí. Los clics y toques necesitan dos permisos: el del paquete (`wprs-ctl input grant
<escena> clicks`) y el de la pantalla (`wprs-ctl input clicks on|off [-o pantalla]`, sin `-o` todas),
que empieza apagado en todas; la app los ofrece en los ajustes de la escena de cada pantalla y la
bandeja en cada pantalla cuya escena los pide. Códigos de salida: 0 bien, 1 error, 2 uso incorrecto, también cuando es el
daemon quien rechaza un argumento (un ajuste o una regla que no existe, un valor fuera de rango).
`wprs-ctl preset rm` acepta rutas con `:` (`~/fotos/a:b.png:Noche`).

La biblioteca vigila sus carpetas, también los enlaces duros y simbólicos y las carpetas movidas o
borradas, y un cambio durante el escaneo de una carpeta trae otro escaneo al terminar. El escaneo
lee los archivos (tamaño, huella rápida y manifiesto de los paquetes) fuera de la base de datos y
guarda los cambios en tandas cortas, sin escribir los archivos que no cambiaron, así que lo que
cambias en la app o la bandeja no espera a que termine de leer: al revisar 1500 archivos sueltos de
1 MiB que cambiaron, otra escritura esperó como mucho 18 ms en vez de 0,73 s, y si no cambió
ninguno la revisión ya no escribe nada (12 ms en vez de 51 ms). Una etiqueta que quitas de un
paquete no vuelve cuando el archivo cambia o llega otra versión (sí si la añades otra vez), y al
renombrar una etiqueta el nombre viejo tampoco vuelve. En una lista aleatoria, *Anterior* y después
*Siguiente* repiten exactamente lo que se mostró, en orden (se recuerdan los últimos 32 fondos),
también después de cambiar la lista o reiniciar el daemon. Un archivo
que el decodificador tarda demasiado en leer (15 s más 1 s por MiB, 2 min como mucho) o que lo hace
caer no queda
marcado como roto: se vuelve a intentar al arrancar el daemon o cuando cambia. Si la biblioteca no
puede guardar algo (disco lleno, base de datos bloqueada), el fondo aplicado se queda y la posición
de una lista se guarda en cuanto se pueda, sin avanzar otra vez. En una lista con intervalo y horas
fijas, un cambio de zona horaria recalcula las horas en la zona nueva y el intervalo sigue contando
desde el último cambio. Si el llavero no responde, `wprs-ctl provider ls` espera como mucho 1 s y
muestra el proveedor como bloqueado hasta que el llavero conteste; durante el minuto siguiente,
nada vuelve a esperarlo. Una imagen se decodifica con límites de tamaño (32768 píxeles de lado y
1 GiB de memoria), los mismos en la biblioteca, el daemon, la app y `wprs-ctl`, así que una foto
que tiene miniatura también se puede poner de fondo; una foto JPEG de hasta 300 megapíxeles se
decodifica directamente a un tamaño reducido, y una cabecera que promete un tamaño enorme se
rechaza al momento en vez de llenar la memoria.

Las ediciones no tocan la imagen: se guardan en `<imagen>.wprs.json` a su lado (o en
`~/.local/share/wallpaper-rs/edits/` si la carpeta no se puede escribir), las aplica la GPU y el
daemon las vuelve a leer en cuanto cambian. La app las edita desde la ficha de cada imagen
(«Editar…»). A la izquierda están siempre a la vista las secciones, cada una con su icono
(*Recortar y girar*, *Pantallas*, *Luz y color*, *Curvas*, *Efectos*, *Máscaras*, *Profundidad* y
*Paralaje*; las flechas recorren la lista), y debajo las *Operaciones, en orden*; al lado, los
controles de la sección elegida, y a la derecha la imagen editada. La dibuja la propia app en la
GPU, con el mismo revelado que el daemon y `wprs-ctl edit export`, reducida (1024 píxeles de lado
como mucho, 512 con render por software) y solo cuando cambia un valor o llega un mapa de
profundidad nuevo; si no puede dibujarla, un aviso dice por qué, y otro avisa si la imagen cambió
después de editarla (también lo dicen `wprs-ctl edit` y el registro del daemon). En *Recortar y
girar* se ve la imagen entera con el recorte encima para moverlo y cambiar su tamaño, en *Máscaras*
las máscaras lineales y radiales se colocan arrastrando sus tiradores sobre la imagen y las de
pincel se pintan sobre ella (con su tamaño, su dureza y *Borrar*; cada trazo es un paso que se
puede deshacer), y en
*Pantallas* el recorte de cada pantalla sobre el resultado y las pantallas dibujadas a escala en
milímetros: se arrastran hasta donde están de verdad, con el ancho de sus marcos, y un patrón de
líneas y círculos sobre las pantallas reales permite comprobarlo (las líneas siguen rectas de una
pantalla a otra). *Deshacer* y *Rehacer* (Ctrl+Z y Ctrl+Mayúsculas+Z) recorren los mismos pasos que
`wprs-ctl edit undo` y `redo`, *Comparar con el original* (tecla `\`) muestra la imagen sin
operaciones y *Exportar…* guarda el resultado en PNG, JPEG, WebP sin pérdida o AVIF. *Ver en el
escritorio* lleva además la edición sin guardar al fondo real, allí donde esa imagen sea el fondo,
y la ventana lo avisa bajo la imagen. Cada acción del editor existe también en `wprs-ctl edit`, que
las cambia sin daemon:

```sh
wprs-ctl edit set foto.jpg ev=0.3 contrast=10 temperature=5200 tint=4
wprs-ctl edit crop foto.jpg 0.1,0.05,0.8,0.9 --rotate 90
wprs-ctl edit crop foto.jpg --aspect 16:9 -o DP-1      # recorte propio de una pantalla
wprs-ctl edit overlay foto.jpg '#ff8800' --blend soft-light --opacity 0.3 --mask linear:0.5,0,0.5,0.5
wprs-ctl edit preview foto.jpg ev=1                  # verlo en el escritorio 60 s sin guardar
wprs-ctl edit export foto.jpg -o foto-editada.png    # mismo resultado, sin daemon
wprs-ctl edit export foto.jpg -o foto.avif --quality 70   # o .jpg y .webp; --before sin operaciones
wprs-ctl edit undo foto.jpg                          # y redo, también tras cambios hechos en la app
wprs-ctl edit set foto.jpg ops.bl.mask=cielo         # dar una máscara a una operación; vacío la quita
wprs-ctl edit mask foto.jpg cara 'brush:0.04,0.6:0.41,0.33;0.43,0.35'   # un trazo de pincel; erase: borra
wprs-ctl edit fit foto.jpg center                    # su propio ajuste; none vuelve al de la pantalla
wprs-ctl set foto.jpg -f span && wprs-ctl edit span foto.jpg --zoom 1.2
wprs-ctl config layout.bezel_mm 8                    # ancho del marco de cada pantalla al extender
wprs-ctl config layout.monitors.DP-1 0,0,597,336     # colocar una pantalla a mano, en milímetros
wprs-ctl calibrate on                                # patrón para comprobarlo; off lo quita
```

Una imagen fija puede moverse en profundidad (paralaje): con un mapa de profundidad en gris (blanco
cerca), el daemon desplaza más lo cercano que lo lejano siguiendo una órbita lenta, el puntero o el
acelerómetro del equipo, solo mientras el fondo se ve, al ritmo que permite la pantalla, con las
mismas pausas que el vídeo y quieto si el sistema pide reducir el movimiento. Elegir el puntero
como fuente para una imagen es tu permiso para seguirlo mientras se ve, igual que en las
escenas. Con render por software (sin GPU) la imagen queda quieta salvo que actives
`playback.software_parallax`, que la mueve con 8 pasos y 15 fotogramas por segundo como mucho.
El mapa puede ser propio (`wprs-ctl edit depth import`, con la misma proporción que la imagen) o
calcularse en el propio equipo con Depth Anything V2 Small (Apache-2.0) mediante ONNX Runtime, en
un proceso aparte y nunca dentro del daemon; gana el mapa más reciente y la caché de mapas
calculados no pasa de 256 MB. Ni el modelo (99 MB) ni ONNX Runtime (11 MB) vienen incluidos: solo
se descargan de GitHub cuando lo pides, después de mostrar tamaño, licencia y servidores, se
comprueban con su SHA-256 fijado, se guardan junto al texto completo de su licencia (y ONNX
Runtime junto a los avisos de terceros de su archivo) y la imagen no sale nunca del equipo.
`models.allow_download = false` impide cualquier descarga y `wprs-ctl models install` instala
archivos obtenidos por otra vía. En la app, al terminar la descarga el mapa se calcula solo; se ve
ya ajustado (cerca, lejos, curva, suavizado y bordes), en gris o en falso color, y el modelo corre
en el procesador o, con un ONNX Runtime compilado con CUDA (por ejemplo indicado en
`ORT_DYLIB_PATH`), en la tarjeta gráfica NVIDIA; *El dispositivo más rápido* la usa cuando puede y
si falla vuelve al procesador:

```sh
wprs-ctl models list                                 # tamaño, licencia, servidor y estado
wprs-ctl models download depth-anything-v2-small     # pregunta antes de descargar
wprs-ctl edit depth generate foto.jpg                # mapa calculado aquí; Ctrl-C cancela
wprs-ctl edit depth generate foto.jpg --device cuda --bench   # medir el modelo en la tarjeta gráfica
wprs-ctl edit depth export foto.jpg mapa.png --adjusted --colour   # el mapa ajustado en falso color
wprs-ctl edit depth foto.jpg --dilate 4 --smooth 2   # ajustar cerca, lejos, curva y bordes
wprs-ctl edit depth remove foto.jpg                  # borrar el mapa calculado, el importado y el de al lado
wprs-ctl edit parallax foto.jpg --source pointer --strength 0.02 --focus 0.3
```

`wprs-ctl pack` (crear, firmar, comprobar, actualizar y validar paquetes `.wpak`) y `wprs-ctl repo`
(claves, firma, publicación, rotación y verificación de un repositorio) funcionan sin daemon. La
contraseña de una clave se pide en la terminal o se lee de `WPRS_KEY_PASSWORD`.

La pestaña *Dispositivos* enlaza el teléfono con tu [servidor propio](#servidor-propio) con el
mismo cliente en Rust que el escritorio, para compartir la biblioteca, las listas y los ajustes con
tus ordenadores. Se escribe la dirección del servidor, *Enlazar este teléfono* muestra un código que
se aprueba en un ordenador ya enlazado (`wprs-ctl server approve` o la página *Cuenta* de la app, con
la contraseña de la cuenta) o en la web del servidor, y después *Obtener la clave* trae la clave de sincronización de otro equipo
comparando seis cifras en las dos pantallas, como entre ordenadores. Desde el teléfono también se
aprueba un equipo nuevo, tras ver qué sistema y qué programa piden el código, y se le comparte la
clave. El token, la clave del teléfono y la de sincronización se guardan cifrados con una clave del
almacén de claves de Android que no sale del teléfono. Los registros van cifrados de extremo a
extremo y el servidor solo ve los equipos como `Device 1`, `Device 2`…

En el teléfono se aplican los títulos, etiquetas, valoraciones, favoritos y ediciones (encuadre,
ajuste, paralaje y el resto de la edición) de los fondos que tiene (el mismo archivo en el teléfono
y en el ordenador es el mismo fondo), las listas con sus reglas y los ajustes compartidos, como el
fundido. Se quedan en el teléfono el volumen, el sonido de los vídeos, los fotogramas por segundo y
el permiso para descargar modelos; los perfiles, los preajustes y los repositorios del
escritorio se conservan sin tocar. Si cambias la clave en un ordenador, el teléfono la recibe solo,
y si se revoca un equipo, el primero que sincroniza (también el teléfono) cambia la clave. La
sincronización en segundo plano usa WorkManager cada hora, solo con Wi-Fi u otra red sin límite de
datos (salvo *Sincronizar también con datos móviles*) y sin batería baja, más una tras abrir la app
y otra 30 segundos después de cada cambio; *Sincronizar ahora* la lanza al momento con cualquier
red. Mientras el teléfono no está enlazado, la app no prepara ninguna tarea ni toca la red al
arrancar. *Pausar la sincronización* y *Quitar el servidor* (que pide al servidor que olvide el
teléfono) están en la misma pestaña.

Mientras la app está abierta, el teléfono escucha el canal de avisos del servidor (el mismo que
usa el escritorio) y sincroniza en cuanto otro equipo guarda un cambio, sin esperar a la hora;
si el canal se corta después de abrirse vuelve a conectarse a los 5 segundos, y solo espera cada
vez más (hasta 15 minutos) cuando el servidor no responde; al salir de la app cierra el canal, y en
segundo plano no queda nada abierto ni ningún despertar nuevo. En la misma pestaña están las funciones de la cuenta que tiene el escritorio: la *Frase de
recuperación* (guardarla, cambiarla o quitarla, con la misma custodia Argon2id que
`wprs-ctl server escrow`, y en un teléfono nuevo sin ningún otro equipo *Recuperar la clave con la
frase*), *Cambiar la clave de sincronización* (como `wprs-ctl server rotate`: cifra todo de nuevo,
la reparte a los demás equipos y vuelve a guardar la frase con la clave nueva salvo que se haya
revocado un equipo), los *Archivos privados* (los fondos propios que otro equipo subió cifrados, con
*Descargar* para añadirlos a la biblioteca del teléfono, y en la página de una imagen o un vídeo
del teléfono *Guardar cifrado en el servidor*, que lo sube por partes cifradas igual que
`wprs-ctl server files share`, y al desactivarlo se borra del servidor), las claves de proveedores
que se sincronizan (el teléfono no usa proveedores, pero las guarda selladas por el almacén de
claves y las muestra) y los *Repositorios*: los que se añaden en otro equipo llegan como
propuesta con su clave y se añaden o se rechazan una vez, también se añade uno por su dirección,
y *Explorar* busca sus paquetes (los marca si son para adultos y los oculta con *Ocultar el
contenido para adultos*) y los instala en la biblioteca, comprobados con el índice firmado
del repositorio; un paquete instalado así es el mismo elemento de la biblioteca en el teléfono y
en el ordenador.

Para sincronizar por la red de casa sin un dominio ni un certificado público, un ordenador puede
servir `wprs-server` con su propio certificado (`WPRS_TLS=self-signed`) y anunciarse en la red
(`WPRS_ADVERTISE=true`, ver [Servidor propio](#servidor-propio)). Mientras se escribe la
dirección del servidor, la pestaña muestra los servidores que encuentra en esa red con el
descubrimiento de servicios de Android. Un servidor con su propio certificado no se acepta a
ciegas: la app muestra la huella SHA-256 de su certificado para compararla con la que da
`wprs-server fingerprint` en el servidor, y solo si eliges *Confiar en él* la fija; desde entonces
el teléfono solo habla con ese servidor si presenta ese mismo certificado, y la pestaña lo dice.

## Paquetes: clasificación, idiomas y créditos

El manifiesto de un paquete (`docs/specs/package.md`) lo describe. En `wallpaper-rs.wpak/2`
declara su clasificación (para todos los públicos o para adultos, con sus motivos), si destella o
muestra patrones rápidos que pueden provocar crisis a personas con epilepsia fotosensible, si es
una obra original o rehecha a mano a partir de otras (con el nombre de sus autores), los créditos
y la licencia de cada archivo, las entradas que puede pedir (sonido, puntero, clics, multimedia y
red), sus enlaces y lo que necesita del motor. El título, la descripción, las notas de cambios y
los textos de sus propiedades pueden ir en varios idiomas, con tablas de cadenas y una cadena de
respaldo (tu idioma, su idioma base, el idioma del paquete); los scripts de una escena reciben el
idioma elegido. La descripción y las notas admiten párrafos, listas, negrita, cursiva y enlaces
`https`, que la app abre solo con un clic y tras confirmarlo; el resto de textos se muestran tal
cual y no pueden llevar `<` ni `>`. Los paquetes de la primera versión siguen funcionando igual,
su descripción se muestra tal cual y aparecen *Sin clasificar*. Las cinco escenas de ejemplo se
describen en inglés y en español, y `pulse` avisa de que destella con el ritmo.

La app y la bandeja difuminan las miniaturas de los paquetes para adultos hasta que decides
mostrarlas (en los detalles de la app o con un clic o el teclado en la miniatura difuminada de una
pantalla, en la página *Ajustes* de la app o en la bandeja; en *Explorar* basta un clic en la
miniatura, y la decisión se guarda para ese paquete), y *Ocultar el contenido para adultos*
(`library.hide_mature`) los saca de las búsquedas y de los repositorios. *Mostrar el contenido para
adultos sin difuminar* (`library.show_mature`, desactivado por defecto) deja de difuminarlas en la
app, la bandeja y el teléfono. Antes de aplicar un
paquete fotosensible, la app y `wprs-ctl` avisan y piden confirmación una sola vez por paquete, y
la app también lo permite desde sus detalles con *Permitir este fondo…* o para varios elementos
seleccionados a la vez con *Permitir estos fondos…*; hasta entonces el daemon no lo aplica, un
perfil no lo pone (la pantalla muestra el motivo y el perfil lo pone en cuanto lo permites) y las
listas lo saltan. La
página *Ajustes* de la app y la bandeja avisan en la pantalla que lo muestra. La búsqueda filtra
por clasificación y por facetas que se calculan del propio paquete: las entradas que usa la escena
(`facet:audio`, `facet:pointer`, `facet:clicks`), el tamaño (`facet:4k`), `facet:photosensitive`,
`facet:android` si funciona entero en el teléfono, etc. Las páginas web toman sus facetas de las
entradas que declaran; en los demás paquetes todas, `media` incluida, salen de lo que usan:

```sh
wprs-ctl lib show 1R1AB2                     # clasificación, origen, créditos, idiomas y descripción
wprs-ctl lib search 'content:general facet:audio'
wprs-ctl lib confirm photosensitive 1R1AB2   # permitir un paquete fotosensible
wprs-ctl lib confirm mature 1R1AB2           # dejar de difuminar su miniatura
wprs-ctl pkg reveal org.example.noche        # lo mismo para un paquete de un repositorio
wprs-ctl pack check fondo/                   # qué necesita, usa y declara, y si lo reproducen el escritorio y el teléfono
wprs-ctl pack upgrade fondo/ --rating general --photosensitive false   # pasar a la versión 2
```

Además de imágenes, vídeos y escenas, el formato describe páginas web (`web`: una página de entrada,
un script que se carga antes que los suyos, un almacenamiento local de 5 MiB como mucho que nunca se
sincroniza, y hasta 8 orígenes `https` a los que puede llegar si das permiso de red, con la API
`window.wprs` para recibir ajustes, sonido, pausa y carpetas), preajustes publicados aparte (`preset`:
dependen de un paquete base por id y rango de versiones y solo llevan valores y sus imágenes o
vídeos), paquetes de recursos que otros autores copian en sus escenas (`assets`, con créditos que
nombran el paquete de origen), dependencias entre paquetes (`requires.packages`), los dispositivos
para los que está hecho (`platforms`), textos en un archivo por idioma (`locale/<idioma>.json`),
paquetes importados para uso personal (nunca se publican) o descargados de un proveedor (con su
crédito y una licencia libre), el color de acento que propone al escritorio, el color alrededor de
la imagen, la extensión sobre todas las pantallas, el volteo, el límite de fotogramas, el inicio en
silencio y la reproducción única. Cada función tiene un nombre en `requires.features`, y el
escritorio y el teléfono dicen si la ejecutan, si reproducen el paquete sin ella o si lo rechazan
con «necesita una versión más nueva de wallpaper-rs» antes de dibujar nada; en esta versión
rechazan los recursos publicados aparte y las texturas KTX2 y AVIF. `wprs-ctl pack check` dice qué hace cada plataforma con un paquete
y `docs/specs/package.md` describe el formato entero.

Un preajuste publicado aparte aparece en la biblioteca como variante de su fondo base: la ficha del
base lista sus preajustes, la del preajuste nombra su base, y *Aplicar* (o `wprs-ctl lib apply`)
muestra el base con esos valores; los que el base instalado no conoce, de otro tipo o fuera de rango
se descartan con un aviso en el registro. Instalar un paquete de un repositorio instala también, del
mismo repositorio, los paquetes que nombra en `requires.packages` y faltan; si aun así falta el base,
la ficha ofrece instalarlo. El autor puede proponer un color de acento (`display.accent`, un color o
una propiedad de color): el escritorio lo da al fondo de Plasma, que lo usa si eliges el acento desde
el fondo, y el teléfono al sistema; sin él se propone el dominante de la vista previa, y
`wprs-ctl state` lo muestra por pantalla. `playback` también vale para escenas (límite de
fotogramas, volumen y silencio del autor): los fotogramas por segundo nunca pasan del mínimo entre
lo que pides, lo que pide el autor, la pantalla, la batería y el nivel de calidad. Las escenas con
sonido, los vídeos y las páginas web tienen su propio volumen por pantalla, que empieza en el del
autor (o en cero si pide empezar en silencio) y se multiplica por el general; *Silenciar los fondos
mientras otra aplicación reproduce sonido* (`playback.duck`, apagado de serie) los baja con un
fundido mientras alguna otra aplicación reproduce, contando sus flujos de PipeWire sin leer sus
nombres ni su contenido. Las propiedades con valores por nivel de calidad (`quality`) los toman solas
cuando el equipo ahorra energía, y scripts y páginas reciben el nivel. Un atajo (`shortcut`) lo
eliges tú en los ajustes del fondo: una dirección `https://` (se abre con el portal del escritorio),
un archivo o una carpeta (con el selector) o una aplicación por su id `.desktop` (fuera del
Flatpak); el fondo nunca ve qué elegiste y solo puede pedir abrirlo dentro de un clic real, como
mucho una vez por segundo, en una pantalla que acepta clics. Si una escena usa clics en una pantalla
a la que no llegan (como Plasma dentro del Flatpak), sus ajustes lo avisan. Un paquete que pide
sombreadores de cálculo avisa antes de aplicarlo si la gráfica no los tiene, y la ficha de cada
paquete dice en qué equipos funciona entero (`desktop`, `android`): el validador lo calcula con las
funciones que usa y los límites del teléfono (1 GiB descomprimido, imágenes de 8192 px de lado y
32 Mpx).

Las escenas también describen ya todo lo que un autor puede querer rehacer a mano: 15 modos de fusión
más (`linear_burn` … `negation`), tinte y brillo de capa, padre de transformación sin cambiar el orden
de dibujo, inclinación en profundidad con perspectiva, máscaras con intensidad, capas que alimentan
efectos, vídeos como máscara o textura, control de reproducción, hojas de sprites y GIF o WebP
animados, filtrado, repetición y mipmaps de texturas, texto con ajuste de línea, alineación vertical,
caja de fondo, contorno, sombra y desenfoque, colecciones de fuentes (`ttc`, `otc`, `woff2`) y
fuentes de respaldo, sonido en MP3, listas de sonidos en orden o al azar con pausas, sonidos largos
en flujo y disparados por eventos, cámara con zoom, giro y sacudida, ajustes del paralaje (también
con la inclinación del teléfono), resplandor de escena, HDR con mapeo de tonos, fotogramas que se dibujan
sobre el anterior, luces puntuales, focos, tubos y direccionales con volumen y proyección,
iluminación 2D con mapas de normales y luz ambiente, captura de una región, eventos con nombre en
las líneas de tiempo, curvas con manejadores, animación por componente y relativa, la hora y la fecha
como valores, rangos de frecuencias con límites y exponente, y efectos propios del paquete en WGSL
con pasadas, objetivos, variantes, hasta 8 texturas y 24 parámetros, comprobados al leer el paquete
contra el contrato de su preludio. Cada uno tiene su nombre en `requires.features`, y el escritorio
y el teléfono rechazan con el mismo aviso una escena que use alguno que su motor aún no dibuja (el
teléfono, que reproduce las escenas sin sonido, solo omite el sonido nuevo). `docs/specs/scene.md`
describe cada campo y sus límites.

De esa lista, el escritorio y el teléfono ya dibujan la cámara 2D (centro, zoom, giro y una
sacudida de ruido que se apaga con «reducir movimiento» y en la calidad más baja), el resplandor de
escena con tinte y su juego HDR (solo en la calidad más alta, o en la siguiente con corriente), los
fotogramas que se dibujan sobre el anterior (más caros, porque cada cambio redibuja la imagen
entera), la inclinación en profundidad de capas y grupos con su máscara proyectada igual, la captura
de una región por una capa de ajuste para cambiarla en su sitio o dibujarla de nuevo como lupa o
espejo, el espaciado entre letras, el contorno, la sombra y el desenfoque del texto desde un campo de
distancias que la GPU construye una vez por cada cambio del texto (y que deja escalarlo sin volver a
dibujar las letras), y los vídeos en cualquier sitio que admite una imagen, con un decodificador por
vídeo que se pausa mientras nada lo muestra, la reproducción de `playback` y el control desde
scripts con `layer.video` (reproducir, pausar, parar, saltar, ritmo, tiempo, duración y aviso de
fin).

El escritorio y el teléfono ya dibujan también los efectos propios del paquete. Cada uno es un
descriptor JSON y un shader WGSL con hasta 16 pasadas que escriben en la siguiente textura o en
objetivos con nombre a su escala (en 8 o 16 bits por canal), los copian, los intercambian para
alternar entre dos (ping-pong), acumulan con fusión por pasada (reemplazar, normal, sumar o
multiplicar), deforman la capa con un shader de vértice sobre una rejilla de hasta 64 × 64 celdas y
leen hasta 8 texturas con su tamaño. Un efecto con `compose` lee la capa ya dibujada sobre lo que
hay debajo y su resultado sustituye esa zona. Las variantes (`options`, constantes `override` de
WGSL) se compilan todas al cargar la escena, nunca a mitad de un fotograma, y los parámetros y las
variantes salen como controles en el editor. El shader recibe además la hora del día, el tiempo del
fotograma, el puntero y su posición anterior, el tamaño de la pantalla, el espectro de 16, 32 o 64
bandas, mezclado o por canal, el paralaje y el color y la opacidad de la capa, con funciones de
ayuda propias (rotación, gris, HSV y los modos de fusión). Una máscara sin imagen toma su color de
`paint`, y una textura puede usar las genéricas `white`, `black`, `noise` o `normal_flat`. Como el
shader viene del paquete, naga lo comprueba antes de que llegue al driver: sin enlaces propios, con
todos los recursos acotados, con bucles de contador entero que avanzan un paso fijo hasta un
límite fijo sin desbordarse, expresiones de como mucho 256 operaciones de profundidad, como mucho
1 048 576 operaciones por píxel, 32 768 operaciones de largo contando cada llamada entera y 16 KiB
de variables por píxel. El shader se compila en hilos aparte con un plazo de 10 segundos. Si el
driver lo rechaza o no termina a tiempo, la capa se dibuja sin el efecto y la app, la bandeja,
`wprs-ctl state` y los diagnósticos del teléfono lo anotan, sin tratarlo como un fallo. Antes de
comprobar o compilar un efecto por primera vez cada programa lo anota como «a prueba» y borra la
nota cuando termina o cuando pasan sus primeros fotogramas; una nota que deja un proceso caído o
colgado cuenta como un fallo, igual que una GPU que se detiene, y con dos el efecto queda fuera
(lo recuerda por el hash del efecto, así que una versión nueva vuelve a probarse). *Probar de
nuevo* en la app, en los diagnósticos del teléfono o `wprs-ctl retry-effects` lo prueban otra vez.
El motor mide en la GPU cada fotograma, cada efecto, también los integrados, y la simulación de las
partículas (`wprs-ctl scene bench` muestra el tiempo como `gpu` y la parte de las partículas en la
línea de la simulación); un efecto de paquete solo queda fuera por lento cuando los efectos de
paquete de un fotograma pasan del límite por sí solos, nunca por lo que cuestan las capas o los
efectos integrados, de eso se encarga el nivel de calidad. Los destinos de los efectos de paquete se
limpian en su primera pasada y las capas de ajuste leen lo que tienen debajo sin copiarlo: una pila
de cuatro efectos de paquete a 4K cuesta alrededor de un 10 % menos.
Cada pasada, cada lectura de textura y cada operación del shader cuentan en el coste de la escena
aunque el descriptor diga menos, y
`wprs-ctl pack check`, `pack build` y el editor avisan cuando una escena no cabe con calidad 2 en
una GPU integrada antigua o de teléfono. `docs/specs/scene.md` explica cómo pasar un shader de HLSL
o GLSL a WGSL, con una tabla de equivalencias.

Las dos plataformas componen ya las capas con los 32 modos de fusión: a los 17 de siempre se suman
`linear_burn`, `linear_dodge`, `darker_color`, `lighter_color`, `vivid_light`, `linear_light`,
`pin_light`, `hard_mix`, `subtract`, `divide`, `reflect`, `glow`, `phoenix`, `average` y `negation`,
con fórmulas publicadas por canal que el motor, los efectos integrados y los del paquete comparten.
Una capa, aunque esté oculta, puede alimentar la textura de un efecto con `{ "layer": "<id>" }`: se
dibuja aparte y entra alineada con la pantalla, de modo que cada punto del efecto lee el punto de la
otra capa que tiene encima, y solo se vuelve a dibujar cuando esa capa cambia. Las máscaras de los
efectos aceptan también una capa o una forma, con inversión, difuminado e intensidad, como las de
las capas, y una pasada puede leer `backdrop`, lo que ya hay dibujado bajo la capa, tanto en los
efectos integrados como en los del paquete. Hay tres efectos integrados nuevos: `tint` tiñe la capa
con un color y un modo de fusión o la colorea conservando su luminosidad, `blend_texture` mezcla
otra imagen o una capa con un modo de fusión, desplazada y repetida, y `light_shafts` dibuja haces
de luz propios, en paralelo, desde un punto o desde una esquina, de un color o con un degradado, y
solo pide fotogramas mientras su velocidad no es cero. Cada capa que alimenta un efecto y cada máscara
dibujada cuentan en el coste de la escena. `docs/specs/scene.md` incluye una tabla de equivalencias
con los efectos de otros programas y cómo convertir sus parámetros.

El escritorio y el teléfono ya dibujan de esa lista el tinte y el brillo de capa, el padre de
transformación, las máscaras con intensidad, los ajustes del paralaje (el teléfono también con su
inclinación, con los mismos sensores que el paralaje de las fotos y solo mientras se ve la escena),
el filtrado nítido, la repetición, el espejo y los mipmaps de las texturas (estos se crean una sola
vez, cuando una imagen se dibuja a menos de tres cuartos de su tamaño), las hojas de sprites y los GIF
o WebP animados (el siguiente fotograma solo se pide cuando toca y nunca con la capa oculta), el texto
con ajuste de línea, número de líneas con puntos suspensivos, interlineado, alineación vertical, caja
de fondo y margen, las colecciones de fuentes, `woff2` y las fuentes de respaldo, las curvas con
manejadores, la animación por componente y relativa, y la hora y la fecha como valores, con los
nombres de meses y días en inglés o en español. Los colores de los fotogramas clave se mezclan en
OKLab por el camino de tono más corto. Los scripts leen además la hora del día (`engine.daytime`),
usan en `Date` la zona horaria del equipo aunque cambie, sin leer ningún archivo, y escriben fechas
con `time.format`.

También dibujan los eventos con nombre de las líneas de tiempo (cada uno salta una vez por pasada,
aunque un fotograma se lo salte, y arranca otras líneas de tiempo o sonidos sin despertar la escena
antes de tiempo) y el cierre de los bucles, que mezcla la última clave con la primera. Los scripts
cambian la velocidad de una línea de tiempo, la paran, leen su tiempo, su duración y si sigue en marcha,
y reciben sus eventos y su final. El escritorio toca las capas de sonido completas: MP3, varios archivos
en orden o al azar con pausas, sonidos largos de hasta diez minutos leídos en flujo sin cargarlos
enteros y sonidos disparados por eventos, con el volumen y el paso de un lado a otro suavizados. El
sonido sigue el tiempo de la escena: los bucles se vuelven a alinear, al reanudar suena donde dice la
línea de tiempo y la latencia baja de unos 160 ms a unos 50 ms. El teléfono sigue reproduciendo las
escenas sin sonido. Una escena sin imagen de vista previa recibe una miniatura dibujada a partir de la
propia escena en su momento de póster.

El formato describe también las escenas más avanzadas: capas `mesh` que deforman una imagen con un
esqueleto (glTF binario con huesos y pesos), con clips y capas de animación, cinemática inversa
hacia un punto, el cursor, un hueso u otra capa, física de muelles y cuerdas con viento, reglas que
cambian el padre de un hueso, formas de mezcla y expresiones, partes con orden y recorte, y huesos
que reciben el puntero; capas `model` con modelos glTF, materiales metálico-rugosos (también pelo,
follaje, croma o shaders de material del paquete), grupos 3D con profundidad compartida, cámara 3D
con recorridos y sombras de las luces; capas sujetas a huesos o puntos de un modelo; sistemas de
partículas en espacio de mundo con puntos de control, ajustes por capa, sistemas hijos, perspectiva,
emisión desde una capa, alrededor o entre puntos, inicializadores con exponente, HSV, paleta,
velocidad vectorial, turbulencia, ruido fractal y reasignación, operadores que oscilan, limitan,
frenan, mantienen distancias o forman bandadas con ventanas de vida, colisiones con cajas, segmentos
y capas, y renderizadores de cuerda, estelas largas, recorte, orientación, luz, refracción y shaders
propios; la declaración de lo que usa cada script (eventos, partes de la interfaz, parámetros,
plantillas de capas y reservas de geometría); valores, carátulas y disparadores del reproductor
multimedia; escenas extendidas sobre todas las pantallas; y
salida a la iluminación RGB con su permiso. Los modelos se leen al abrir el paquete y cada hueso,
animación, parte o material que nombra la escena se comprueba contra ellos. Las entradas de un
paquete de la versión 2 salen solo de lo que declara: también las de sus scripts, que ya no se
adivinan por su texto; un script de la versión 1 que no declara nada usa el sonido o el puntero solo
si importa `audio` o `input` de `wprs`, nunca porque esas palabras salgan en un comentario. Todo tiene su nombre en `requires.features` y, mientras el motor no lo dibuja,
el escritorio y el teléfono rechazan la escena con el aviso de versión más nueva (el teléfono, que no
reproduce sonidos de escena ni tiene iluminación RGB ni varias pantallas, solo omite esas partes).
Las capas `solid` ya reciben el puntero: `click:`, `enter:` y `leave:` arrancan líneas de tiempo y
sonidos sin scripts, y los scripts reciben los eventos de cada capa; los scripts también reproducen,
pausan y paran las capas de sonido.

Las capas `mesh` ya se dibujan en el escritorio y en el teléfono: la GPU deforma los vértices con
hasta cuatro huesos cada uno y las formas de mezcla más pesadas, y la imagen resultante se compone
como cualquier otra capa, con sus efectos, máscaras y modos de fusión. Los clips toman las
animaciones del archivo o claves propias en el JSON de la escena (giro, posición y escala de cada
hueso, peso de cada forma y orden de cada parte), en bucle, ida y vuelta o una vez; las capas de
animación se mezclan con peso, velocidad, fundidos de entrada y salida, capas aditivas y capas
limitadas a unos huesos. Las reglas pasan un hueso a otro padre sin que salte, las expresiones
agrupan formas, y las partes se ordenan y se recortan unas sobre otras. Un esqueleto quieto o que ya
terminó no pide fotogramas, la pose se calcula solo para capas visibles y sin reservar memoria en
cada fotograma, y en la calidad más baja la malla se dibuja a la mitad de resolución y se mueve a 30
fotogramas por segundo como mucho. Los scripts controlan los clips con
`scene.layer("zorro").anim("saludo")` (`play`, `pause`, `stop`, `seek`, `rate`, `weight` y el aviso
`ended`) si declaran `animations`.

También se mueven solas: las cadenas de cinemática inversa giran sus huesos hasta alcanzar un punto,
un hueso, otra capa o el puntero (este solo con el permiso del puntero, que la app avisa como
cualquier otro uso del puntero), exactas para dos huesos y por rondas para cadenas más largas, con
límites por hueso, peso y sentido del codo. La física de huesos simula muelles, péndulos y cuerdas a
un paso fijo de 1/60 s (1/30 s con batería y como mucho 15 pasos por fotograma), con inercia,
amortiguación, gravedad, límites de giro y de estiramiento y el viento de `canvas.wind` con ráfagas
(`canvas.gusts`, de 0 a 1);
un cuerpo quieto medio segundo deja de pedir fotogramas y en la calidad más baja los huesos se
quedan en la pose de la animación. Con `depth` la profundidad de los vértices se proyecta con la
perspectiva de `canvas.perspective` (también bajo la inclinación de la capa) y las caras vistas por
detrás se ocultan si se pide. Cualquier capa se sujeta a un hueso o a un nodo de una malla con
`attach`, siguiendo su posición, giro y escala del mismo fotograma. Los scripts leen y mueven huesos
con `scene.layer("zorro").bone("brazo")` (giro, posición y escala locales o del mundo, para el
fotograma siguiente y antes de la física, de modo que el hueso vuelve con su muelle al soltarlo),
fijan formas de mezcla con `shape`, empujan huesos con `impulse`, buscan el hueso más cercano con
`hit` y leen puntos de sujeción con `attachment`; los huesos de `hit` reciben el puntero y los
eventos dicen qué hueso se tocó.

De las partículas nuevas, el escritorio y el teléfono ya dibujan, igual en la GPU que en la CPU, el
espacio de mundo (las partículas nacen con la posición, el giro y la escala de su capa y después no
la siguen), hasta ocho puntos de control movidos por valores, el cursor, la capa o los scripts, las
cajas huecas y las elipses o medios círculos con un punto de anclaje, la duración, el retardo, la
emisión por ventanas aleatorias, el tope de una partícula por paso y la espera a que suene algo, los
inicializadores con exponente, alfa propio, HSV, paleta, velocidad vectorial, velocidad hacia fuera
y velocidad heredada del cursor o de un punto, los operadores con ventanas de vida (oscilar posición,
alfa o tamaño, limitar la velocidad, par y freno de giro, frenar cerca de un punto, alcance y caída
de atractores, vórtices con anillo y turbulencia con desplazamiento y máscara), los sistemas hijos
estáticos, al nacer, al morir o que siguen a cada partícula (dos generaciones como mucho), los ajustes
por capa (alfa, tamaño, cantidad, velocidad, vida, ritmo, brillo, color y puntos, con bloqueo en el
sistema), el brillo, los ciclos y el estiramiento acotado del dibujo, y el control desde scripts con
`particles(capa)`: `play`, `pause`, `stop`, `playing`, `emit(n)`, `point(n, [x, y])` e `instance`.
Una escena simula como mucho 64 sistemas contando hijos y capas con partículas propias, dentro de
las 200 000 partículas, y el precalentamiento llega a 30 s en pasos de 1/15 s. Lo demás de la lista
anterior sigue pidiendo una versión más nueva.

También dibujan las estelas con historial (cada partícula guarda sus últimas posiciones, hasta 16
pasos de 1/60 s, y deja una cinta que se estrecha y se apaga con el color y el tamaño que tenía en
cada punto; 4, 8 o 16 pasos según la clase de equipo, menos o ninguno en los niveles de calidad
bajos y ninguno con render por software), el orden de dibujo (`render.order`: las más nuevas, las
más viejas, las más grandes, las más pequeñas o las más bajas encima, en 256 escalones), los choques
contra la parte opaca de una imagen (`collide` con `mask`, convertida al cargar en un campo de
distancias de hasta 16 384 celdas), el color inicial en degradado (`init.gradient`, hasta 8
paradas), las hojas de sprites con rectángulos de distinto tamaño, y los extremos de los rangos
iniciales y el radio de los círculos enlazados a valores, que se leen en cada fotograma. Los enlaces
de sonido toman rangos de bandas en 16, 32 o 64 (`audio: [0, 3]`, `of`, `reduce: mean | max`) de un
canal, límites (`bounds`), exponente y suavizado distinto al subir y al bajar (`smooth: { up, down }`),
también para el cursor, las propiedades y los scripts, y una escena puede fijar con `audio` su ataque,
su caída y si sigue el volumen del sistema (`gain: fixed`) o se ajusta sola (`auto`). Cuando las
partículas pasan al procesador, el estado de la pantalla lo dice (`"particles": "cpu"`) y la
aplicación, la bandeja, `wprs-ctl` al mostrar la escena y los ajustes del teléfono avisan de que se
muestran menos.

Además dibujan, igual en la GPU que en la CPU, la perspectiva de las partículas en escenas 2D
(`perspective` con la profundidad y su velocidad al nacer, de lejos a cerca con fusión normal), el
recorte del alfa de la textura (`render.cutout`), la orientación hacia la pantalla, el movimiento o un
ángulo fijo (`render.orient`), la emisión desde los píxeles visibles de otra capa con su color y su
movimiento (forma `layer`, leída a 256 o 512 píxeles y de nuevo solo cuando la capa cambia), la
velocidad inicial turbulenta, el desfase fractal del nacimiento, el nacimiento por turnos alrededor
de un punto o entre dos con arco y extremos afinados, la reasignación de valores al nacer
(`init.remap`, hasta ocho, con la hora, el tiempo, la vida, la velocidad, el azar, la distancia a un
punto, el sonido o el puntero), la herencia del color, el tamaño, el alfa y la velocidad de la
partícula padre, y los shaders de partículas de un efecto del paquete (`render.shader`, con las mismas
comprobaciones que los demás efectos del paquete y dejado fuera, sin dejar de dibujar las partículas,
si no se puede construir).

El escritorio y el teléfono dibujan también la mezcla por degradado, el revelado con el puntero,
el desplazamiento por mapa o por un ruido propio, las cáusticas, los destellos, el fuego, el giro,
la transformación, el pulso (también al ritmo de una banda de sonido), el contraste local, la estela
de movimiento, el VHS, los fluidos, el destello de lente y el chisporroteo eléctrico, y los parámetros
nuevos de los efectos que ya existían: rayos de luz direccionales o en estrella, aberración cromática
en cuatro modos, grano en blanco y negro o con otro modo de fusión, desenfoque mezclado con la imagen,
fondo visible en el iris y la lente, reflejo inclinado, desplazamiento sin repetición, sacudida con
ruido, fricción y límites, brillo con fusión y degradado, balanceo doble, remolino elíptico,
ondulaciones con perspectiva, reflejo, tinte, paredes y velocidad, ondas dobles con perspectiva y un
desfase por píxel (`phase`) que evita que las zonas vecinas se muevan a la vez. Con los valores por
defecto todo se ve exactamente como antes. Los efectos que dependen del tiempo dejan de pedir
fotogramas cuando su velocidad es cero; la estela y los fluidos se detienen en cuanto se asientan;
los fluidos se congelan en la calidad más baja, y los efectos medios se omiten en ella, como al
funcionar con batería. Los modos de fusión `tint` y `diffuse_light` son aproximaciones cuyas fórmulas
pueden cambiar. Se nombran `effect.<id>` y `effect.<id>.<parámetro>`, de modo que los motores
anteriores rechazan con el aviso de versión más nueva los paquetes que los usan. Las líneas de
tiempo pueden medirse en fotogramas (`frames` y `fps`, que ya funciona en las dos plataformas) y
enlazar el final de un bucle con su principio (`wrap`, rechazado mientras el motor no lo dibuje), y
`"seed": null` da a una escena una semilla nueva cada vez que empieza. Un paquete con un vídeo HDR
(PQ o HLG) recibe la faceta `hdr` y `wprs-ctl pack check` lo dice: en el ordenador se decodifica con
10 bits y la gráfica adapta sus tonos a colores normales (BT.2020 a sRGB, el blanco de la pantalla
a 203 nits y la luz más fuerte a 1000), también dentro de las escenas, y los vídeos normales no
cambian. Los clientes de repositorios se saltan las versiones de un tipo de paquete que no conocen
en vez de dejar de leer el índice. La tabla de perfiles (qué hacen el escritorio y el teléfono con
cada función) está en `docs/specs/package.md`, y `docs/specs/scene.md` termina con una guía para
rehacer a mano una escena hecha con otro programa: dónde va cada parte en el formato y cómo pasar
las funciones de HLSL y GLSL a WGSL.

`wprs-ctl pack build` calcula las funciones del motor que usa el paquete (`requires.features`) y las
escribe en el manifiesto; si un paquete pide una función o una versión que este wallpaper-rs no
conoce, dice que hace falta una versión más nueva (en el escritorio, con su miniatura). Las
versiones publicadas antes de la versión 2 del manifiesto no abren sus paquetes, pero siguen leyendo
los repositorios que los tienen: solo ven en ellos los de la primera versión. Las páginas web, los
preajustes y los paquetes de recursos van solo en los índices nuevos del repositorio, que esas
versiones no leen, así que no los ven. El validador comprueba
que las entradas declaradas incluyan las que la escena puede usar con cualquier valor de sus
propiedades y dice dónde las usa. Después de actualizar wallpaper-rs, la biblioteca vuelve a leer
una vez sus paquetes, así que los que pedían una versión más nueva empiezan a funcionar y los
anteriores reciben sus facetas. En el teléfono, la página de un paquete avisa de lo que no funciona
allí.

## Fondos web

Un paquete `web` es una página (HTML, CSS, JavaScript, WebAssembly, WebGL, imágenes, fuentes,
sonido y vídeo dentro del propio paquete) que se muestra como fondo con las mismas transiciones,
ajuste, límite de FPS y pausas que los demás. La dibuja `wprs-web`, un proceso aparte que el
servicio arranca para cada página que se ve y para en cuanto ninguna pantalla la muestra. Necesita
WPE WebKit 2.54 o posterior (`wpewebkit` en Arch y Fedora); donde no está (Ubuntu 24.04, el Flatpak)
el fondo se rechaza con el motivo en lugar de quedarse en blanco. `docs/specs/web.md` explica el
motor, el aislamiento y la interfaz de la página, y `docs/guia-fondos-web.md` es la guía para quien
los hace.

La página solo lee los archivos de su paquete, no llega a la red salvo que la declare y la permitas
(`wprs-ctl input grant <id> network`, o el interruptor de sus detalles en la app), no ve rutas,
dispositivos ni la batería, y el motor web la ejecuta en su propio entorno aislado. Por defecto ve
tu zona horaria, tu idioma, lo que puede hacer tu tarjeta gráfica (WebGL) y guarda sus datos en
`~/.local/share/wallpaper-rs/web/<id>/` si los declara. Al aplicarla y en sus ajustes, la app, la
bandeja y `wprs-ctl` avisan de qué usa, y puedes ocultar cada cosa por fondo: en los detalles del
paquete en la app (*Privacidad*) o con
`wprs-ctl lib confirm hide_zone|hide_language|hide_gpu|hide_storage ELEMENTO` (`lib unconfirm` lo
deshace). *Borrar datos* en la app o `wprs-ctl lib clear-data ELEMENTO` borra lo que guardó, y se
borra solo al quitar el paquete. Los permisos de sonido, puntero y clics son los mismos que los de
las escenas: los clics solo llegan desde clics reales en las pantallas donde los activas. Una página
oculta deja de dibujar al momento y, tras 5 minutos oculta, se detiene su proceso y queda su último
fotograma. El ejemplo *Luces* (`scenes/lights`) es una página web.

En el teléfono los mismos paquetes se muestran con el WebView del sistema, el motor que Android
mantiene al día para todas las apps, dibujado directamente sobre el fondo sin copias. La página ve
la misma interfaz y tiene las mismas restricciones: solo lee sus archivos y los que eliges (la app
sirve cada petición), no llega a la red salvo que la declare y la permitas, no tiene cookies,
ventanas, diálogos, descargas ni permisos, y se le quitan las conexiones entre pares (WebRTC) antes
de que corran sus scripts. Su página en la app avisa, encima de *Usar como fondo*, de lo que
usa (zona horaria, idioma, modelo del chip gráfico y datos que guarda en el teléfono), y su sección
*Privacidad* tiene un interruptor para cada cosa, otro para la red si la declara y *Borrar datos*;
el mismo aviso sale al importarla. Ocultar la zona horaria y el idioma lo hace un script que corre
antes que los de la página (en ella y sus marcos, no en sus workers). *Reaccionar al tacto* le da
el puntero y *Reaccionar a los toques* los clics, solo desde toques reales y solo en la pantalla de
inicio; su sonido sigue el ajuste de sonido de los vídeos. Se pausa al dejar de verse, con el ahorro
de energía, con poca batería o con el teléfono muy caliente, se cierra tras 5 minutos oculta y
`requestAnimationFrame` va como mucho a los fotogramas por segundo del teléfono; el cambio a una
página o desde ella no tiene fundido. En un emulador, *Luces* dibuja 30 fotogramas por segundo con
el límite de 30, una página fija no gasta nada después de su primer fotograma y una página oculta
no dibuja ni usa el procesador.

## Servidor propio

`wprs-server` publica un repositorio firmado de paquetes, un catálogo web sin JavaScript, cuentas
con subidas y moderación, y la sincronización de la biblioteca, las listas y los ajustes entre
dispositivos, cifrada de extremo a extremo por defecto. Es un solo binario en un contenedor sin
shell, con un usuario sin privilegios y la raíz de solo lectura; todos los datos viven en el
volumen `/data`. No envía telemetría ni se conecta a terceros.

```sh
docker compose up -d                          # construye la imagen desde este repositorio
docker compose logs wprs | grep setup         # token de un solo uso del primer administrador
docker compose exec wprs wprs-server repo init --name "Fondos de casa"
```

En `http://localhost:8080/setup` se pega el token y se crea el administrador (si el registro ya
no lo muestra, `docker compose exec wprs wprs-server setup-token` da otro). `repo init` crea las
claves del repositorio y muestra una sola vez la clave raíz, que hay que guardar fuera del
servidor; el servidor solo conserva la clave en línea, con la que vuelve a firmar el índice cada
día (cada firma caduca a los 7). Desde *Administración* se crean invitaciones y desde *Cuenta*,
tokens, dispositivos enlazados con un código y subidas de paquetes `.wpak` de las dos versiones del
manifiesto. El servidor acepta los paquetes para adultos que lo declaran en su manifiesto: llevan
la etiqueta `mature` en el catálogo, en la búsqueda (`content=mature`, `facet=mature`) y en la API, y
la web difumina sus miniaturas hasta que eliges verlas, una por paquete y sin JavaScript (la web no
guarda la elección, así que vuelve a difuminarlas en cada visita). El
catálogo filtra por clasificación y por faceta y avisa en la página de un paquete fotosensible.

Los moderadores pueden ocultar un paquete sin retirarlo, con un motivo que ve su autor: sale del
catálogo y de la búsqueda, pero quien lo tiene instalado lo conserva y recibe sus actualizaciones.
Se hace desde *Moderación* en la web, con la API o con
`wprs-ctl moderate --server https://fondos.example.org hide org.example.fondo --reason "…"`
(`unhide` lo vuelve a mostrar y `hidden` los lista), con un token personal con el permiso `mod` en
`WPRS_SERVER_TOKEN`.

Con `repo init --online-authors`, cada cuenta registra en *Cuenta* las claves con las que firma sus
paquetes, sin pasar por la clave raíz. Cada cliente se fía de la clave que firmó la primera versión
que instaló de cada paquete y solo acepta otra si la anterior la autorizó: `wprs-ctl pack rotate-key
vieja.key nueva.pub --author u<id>` firma ese traspaso, que se guarda en *Cuenta*. Así, quien robe la
clave en línea del servidor puede añadir autores y paquetes nuevos, pero no actualizar un paquete
ya instalado. Los autores que lista la raíz tienen prioridad.

El servidor construido así se identifica con la versión de `Cargo.toml` y el sufijo `-local`. Para
que muestre la de una versión publicada, se construye desde su etiqueta:

```sh
WPRS_VERSION=$(git describe --tags --exact-match --match 'v[0-9]*' | cut -c2-) docker compose up -d --build
```

Cada versión publica también la imagen en `ghcr.io/wallpaper-rs/wprs-server` (`linux/amd64`, con las
etiquetas `<versión>` y `latest`), firmada con cosign sin clave propia como los archivos de
*Releases*. Lleva adjuntos los SBOM SPDX de BuildKit (los paquetes de la imagen y de la etapa de
compilación) y su procedencia, y una atestación SPDX firmada con las dependencias de Rust de las
fuentes con las que se construyó. Para usarla en lugar de construirla se fija su resumen, se
comprueban la firma y la atestación y se arranca con `deploy/compose.image.yaml`:

```sh
IMAGE=ghcr.io/wallpaper-rs/wprs-server
DIGEST=$(docker buildx imagetools inspect "$IMAGE:<versión>" --format '{{ .Manifest.Digest }}')
ID=https://github.com/wallpaper-rs/wallpaper-rs/.github/workflows/ci.yml@refs/heads/master
ISSUER=https://token.actions.githubusercontent.com
cosign verify "$IMAGE@$DIGEST" --certificate-identity "$ID" --certificate-oidc-issuer "$ISSUER"
cosign verify-attestation "$IMAGE@$DIGEST" --type spdxjson \
  --certificate-identity "$ID" --certificate-oidc-issuer "$ISSUER" > wprs-server.sources.spdx.dsse
docker buildx imagetools inspect "$IMAGE@$DIGEST" --format '{{ json .SBOM }}' > wprs-server.sbom.json
WPRS_IMAGE=$IMAGE@$DIGEST docker compose -f compose.yaml -f deploy/compose.image.yaml up -d
```

La configuración va en variables de entorno, en un `.env` junto a `compose.yaml`; `VARIABLE_FILE`
lee el valor de un archivo, para los secretos:

| Variable | Por defecto | Uso |
|----------|-------------|-----|
| `WPRS_PUBLIC_URL` | `http://localhost:8080` | URL pública del servidor |
| `WPRS_BIND` | `127.0.0.1:8080` | Dirección y puerto que publica compose |
| `WPRS_TRUSTED_PROXIES` | vacía | Redes del proxy inverso cuyo `X-Forwarded-For` se cree; sin ellas, los límites de peticiones ven a todos los clientes del proxy como uno solo |
| `WPRS_REGISTRATION` | `invite` | `closed`, `invite` u `open` |
| `WPRS_AUTO_PUBLISH` | `trusted` | Publicar sin revisión: `never`, `trusted` o `all` |
| `WPRS_LICENSES` | `CC0-1.0,CC-BY-4.0,CC-BY-SA-4.0` | Licencias aceptadas |
| `WPRS_LEGAL_OPERATOR`, `WPRS_LEGAL_CONTACT` | vacías | Responsable y contacto de las páginas legales |
| `WPRS_OIDC_ISSUER`, `WPRS_OIDC_CLIENT_ID`, `WPRS_OIDC_CLIENT_SECRET` | vacías | Inicio de sesión único opcional |
| `WPRS_LOG_CLIENTS` | `off` | IP en un registro de abusos: `off`, `prefix` o `full`; se borra a los `WPRS_LOG_CLIENTS_DAYS` días (7) |
| `WPRS_NOTICE_DAYS` | `180` | Días que un aviso legal guarda el nombre, el correo y la firma de quien lo envía |
| `WPRS_TAKEDOWN_DAYS` | `90` | Días que se guarda fuera de la vista pública un paquete retirado, como prueba y para una contranotificación |
| `WPRS_TLS` | `off` | `self-signed`: sirve HTTPS con un certificado propio, que los clientes aceptan solo por su huella fijada |
| `WPRS_ADVERTISE` | `false` | Anunciar el servidor en la red local por mDNS (`_wprs._tcp`) con un nombre al azar en cada arranque |

La lista completa de variables y la API HTTP están en
[`docs/specs/server.md`](docs/specs/server.md).

### Enlazar los equipos

El servicio de cada equipo se enlaza con el servidor desde *Cuenta* en la app o con la CLI:

```sh
wprs-ctl server add https://fondos.example.org
wprs-ctl server add https://192.168.1.10:8443 --fingerprint <huella>   # un servidor de casa con su propio certificado
wprs-ctl server link --name Escritorio          # muestra un código para /link o para:
wprs-ctl server approve BCDF-GHJK                # … en un equipo ya enlazado, tras ver quién lo pide y con la contraseña
wprs-ctl server join "Device 1" --wait           # equipo nuevo de una cuenta cifrada: muestra sus cifras y pide las del otro
wprs-ctl server share "Device 2"                 # en el equipo con la clave: muestra sus cifras y pide las del otro
wprs-ctl server status
wprs-ctl server rotate                           # clave nueva; los demás equipos la reciben solos
wprs-ctl server reset                            # sin ningún equipo con la clave: rehace los datos desde este
wprs-ctl server upload fondo.wpak                # sube un paquete con el token del equipo
wprs-ctl server repo 1a2b3c4d                    # añade un repositorio que otro equipo sincronizó, tras revisarlo
wprs-ctl server secret wallhaven                 # sincroniza la clave de un proveedor, cifrada
wprs-ctl server files share ./fondos             # sube tus propios fondos, cifrados; en otro equipo:
wprs-ctl server files ls                         # … los lista y `server files fetch <id>` descarga uno
wprs-ctl server escrow                           # guarda la clave en el servidor, cerrada con una frase
wprs-ctl server restore                          # equipo nuevo sin ningún otro: recupera la clave con la frase
```

Aprueba solo un código que muestre la pantalla de tu propio equipo nuevo: quien te envía un código
intenta entrar en tu cuenta. Aprobar desde un equipo pide la contraseña de la cuenta, porque los
tokens de los equipos ya no gestionan dispositivos: quien robe uno no puede añadir equipos ni
revocar los demás (una cuenta con inicio de sesión único aprueba en `/link`). Las dos pantallas muestran el mismo número de seis cifras solo si
nadie se ha colado entre los equipos, y en cada uno escribes las cifras del otro: la clave no sale
ni se guarda hasta que coinciden en los dos; cada intento da una única oportunidad a quien lo
intente. Al revocar un equipo, otro cambia la clave de sincronización y vuelve a cifrar todo, así
que el equipo revocado ya no puede leer lo nuevo; la clave nueva pasa solo entre equipos que
compararon sus cifras, y los demás vuelven a unirse. Cambiar la contraseña en la web cierra las
demás sesiones y borra los tokens; los equipos siguen enlazados salvo que elijas cerrarlos también.

Se sincronizan los títulos, etiquetas, valoraciones, favoritos y ediciones de las imágenes de la
biblioteca (cada parte de la edición por separado, así que lo que cambian dos equipos en partes
distintas se suma, y una parte de más de 64 KiB conserva la última copia enviada), las listas, los
perfiles, los preajustes y los ajustes (salvo el volumen, la caché, el paralaje por
software y el permiso para descargar modelos, que se quedan en cada equipo), cifrados de extremo a
extremo; sin cifrar solo con `--plaintext`, que avisa antes. El token y la clave de sincronización
viven solo en el llavero del sistema; el servidor ve los equipos como `Device 1`, `Device 2`… y el
nombre real viaja cifrado. Al enlazar se añade también el repositorio del servidor.

Los repositorios que añade un equipo llegan a los demás como propuesta: cada equipo los añade solo
cuando lo confirmas, con su huella. Las claves de los proveedores viajan solo si lo eliges para
cada proveedor, solo en cuentas cifradas, y en cada equipo se guardan solo en su llavero. Tus
propios fondos (no los de un repositorio) suben cifrados por partes si eliges el archivo o su
carpeta; si se corta, la subida sigue donde se quedó, el mismo archivo se guarda una vez y los
demás equipos lo descargan cuando lo pides. Si dos equipos cambian una lista de formas que no se
pueden juntar, se conservan las dos: la otra queda como «Lista (conflicto)». Lo que una versión
más antigua no entiende se le devuelve tal cual, y el servidor recuerda la versión de formato más
nueva que escribió en la cuenta: un equipo con una versión anterior deja de sincronizar y pide que
lo actualices, en lugar de subir nombres viejos o registros sin los campos que no conoce.

Borrar un elemento de la biblioteca en un equipo lo borra también en los demás (desinstalando el
paquete), sin tocar sus archivos; un elemento de una carpeta vigilada cuyo archivo sigue ahí se
queda, pero no vuelve a los demás. Los borrados se recuerdan 90 días y luego el servidor los purga.
Si un cambio choca tres veces seguidas o la sincronización falla con cambios pendientes, se
reintenta a los 30 s, 1, 2, 4 y 8 minutos y después cada 15, sin preguntar al servidor si no ha
cambiado nada. Si un archivo propio no se puede subir (cuota llena o archivo desaparecido), los
demás se suben igual y el error nombra cada uno que falló.

Los nombres de los elementos en el servidor son hashes con una clave de nombres que no cambia al
rotar la clave de sincronización: un equipo que la tuvo, aunque se haya revocado, y tenga el mismo
archivo puede saber que tu cuenta lo guarda. La app, `wprs-ctl server --help` y el teléfono lo
explican.

Si temes perder todos los equipos, `server escrow` guarda la clave en el servidor cerrada con una
frase (Argon2id, 256 MiB). Quien consiga una copia del servidor puede probar frases sin límite: usa
varias palabras sin relación que no uses en ningún otro sitio. Tras revocar un equipo la frase deja
de valer: la app, `wprs-ctl server status` y el teléfono lo dicen, y hay que guardar otra nueva y
distinta, porque quien conocía la anterior podría abrir lo que se cierre con ella; la anterior se
rechaza.

Copias de seguridad: `docker compose exec wprs wprs-server backup` deja en
`/data/backups/<fecha>` una instantánea coherente de la base de datos (`VACUUM INTO`) con los
paquetes, miniaturas y archivos privados que nombra, las claves y el repositorio, con la suma SHA-256 de cada archivo. Para
restaurar, con el servidor parado:

```sh
docker compose cp wprs:/data/backups ./backups             # sacar las copias del volumen
docker compose stop wprs
docker compose run --rm wprs restore /data/backups/<fecha> --force
docker compose up -d
```

La restauración comprueba las sumas y la integridad de la base de datos, firma un índice nuevo
para que los clientes no vean una versión anterior y hace que cada dispositivo vuelva a descargar
lo sincronizado.

En una red de casa sin dominio, el servidor puede servir HTTPS él mismo con un certificado propio
y anunciarse en la red local para que el teléfono lo encuentre:

```sh
WPRS_TLS=self-signed WPRS_ADVERTISE=true WPRS_LISTEN=0.0.0.0:8443 \
  WPRS_PUBLIC_URL=https://192.168.1.10:8443 wprs-server serve
wprs-server fingerprint                    # la huella SHA-256 del certificado, también en el registro
wprs-ctl server add https://192.168.1.10:8443 --fingerprint <huella>
```

El certificado se crea en el primer arranque en `/data/tls` y se conserva. Los equipos y el
teléfono lo aceptan solo por su huella, que se compara una vez y queda fijada: sin ella, `server
add` falla y dice la huella que mostró el servidor; en la app se escribe en *Añadir servidor*, en
el teléfono se confirma en un aviso, y `wprs-ctl server status` y la app la muestran. El anuncio
(`_wprs._tcp` por mDNS) solo dice que hay un servidor de wallpaper-rs, con un nombre al azar que
cambia en cada arranque, y nada de la cuenta ni de los equipos. Es la misma sincronización y el
mismo cifrado de extremo a extremo que con un servidor público, solo que sin salir de la red de
casa.

Con dominio propio y certificado automático, Caddy delante:

```sh
WPRS_DOMAIN=fondos.example.org docker compose -f compose.yaml -f deploy/compose.caddy.yaml up -d
```

Caddy obtiene el certificado y reenvía las peticiones; el servidor solo cree `X-Forwarded-For`
cuando llega desde la red de los contenedores.

Los paquetes de escena (`docs/specs/scene.md`) se importan y se aplican como cualquier otro
elemento de la biblioteca. El daemon los dibuja en la GPU solo cuando algo visible cambia, con
menos fotogramas y menos calidad con batería o con render por software; las escenas con
`"clock": { "max_fps": "display" }`, como las de ejemplo, siguen la frecuencia de la pantalla:

```sh
wprs-ctl prop ls -o DP-1                     # propiedades de la escena que muestra DP-1
wprs-ctl prop set tint '#ff8800' -o DP-1     # se aplica al momento y se guarda en config.toml
wprs-ctl prop set volume 60 -o DP-1          # volumen propio de una escena con sonido
wprs-ctl prop press again -o DP-1            # pulsa un botón de la escena
wprs-ctl prop share -o DP-1                  # los mismos valores en todas las pantallas (--own: solo esta)
wprs-ctl prop preset load calma -o DP-1 && wprs-ctl prop reset -o DP-1
wprs-ctl prop preset save Noche -o DP-1      # guarda los valores actuales como preajuste propio
wprs-ctl prop preset export Noche -f noche.json -o DP-1
wprs-ctl prop preset import noche.json --save Noche -o HDMI-A-1
wprs-ctl scene bench escena.wpak --size 1920x1080 --frames 300   # coste por fotograma, sin daemon
wprs-ctl scene bench escena.wpak --screens 2   # carga, primer fotograma y memoria con dos pantallas
```

En la página *Ajustes* de la app, el botón *Ajustes de la escena* (o *Ajustes del vídeo*) de cada
pantalla cambia sus propiedades al momento, aplica sus preajustes, guarda, renombra y borra los
propios, copia y pega los ajustes o los exporta e importa como un archivo pequeño, decide si la
pantalla comparte sus valores con las demás, da o retira el permiso de sonido, de puntero y de
clics y enciende o apaga los clics de esa pantalla. Los preajustes propios son del paquete (siguen tras actualizarlo) y viajan con la sincronización.

Los autores describen sus propiedades en el manifiesto (`docs/specs/scene.md`): colores con
transparencia, deslizadores con paso, decimales, unidades, escala logarítmica y casilla para
escribir el número, listas con iconos y etiquetas traducidas, textos y textos secretos (que se
guardan en el llavero del sistema y nunca llegan a `config.toml`, a los registros ni a la
sincronización), separadores, etiquetas y grupos plegables, botones que lanzan eventos de la escena,
y archivos y carpetas que elige el usuario con el selector de archivos, también vídeos, que
sustituyen una imagen, un vídeo, una máscara o la textura de unas partículas del paquete sin
reiniciar la escena. Cada propiedad puede mostrarse o activarse según las demás con condiciones
como `mood == 2 || speed > 1.5`, de un lenguaje pequeño que se comprueba al validar el paquete y
que no ejecuta código. Los scripts nunca ven las rutas de los archivos elegidos. Una carpeta solo
se lee mientras la escena se ve, puede incluir hasta dos niveles de subcarpetas y limitar cuántos
archivos usa; `wprs-ctl prop set` acepta rutas relativas a la carpeta actual. El formato también
describe atajos que el usuario elige para abrir con un clic y valores que cambian con el nivel de
calidad. Mientras una pantalla comparte los valores con las demás, lo que
se cambia en ella cambia en todas. Un paquete de vídeo ofrece velocidad (sin cambiar el tono ni
dibujar más fotogramas), el tramo que se repite, volumen propio, zoom y posición, con los valores
por defecto que elige el autor, y se aplican sin volver a empezar el vídeo.

La carpeta `scenes/` guarda cinco escenas de ejemplo de dominio público (CC0):
capas, modos de fusión, máscaras, línea de tiempo y profundidad que sigue al puntero o una órbita
lenta con un mapa calculado a partir de sus formas (`dusk`), partículas, efectos, el sonido del
viento y las imágenes de una carpeta que elijas tras la nieve (`snowfall`), análisis del sonido
(`pulse`), puntero y el sonido del agua (`pond`) y un script con una imagen o un vídeo de fondo que
eliges (`orbits`). `scripts/ambient-sounds.sh` sintetiza esos dos sonidos (ruido filtrado y gotas,
también CC0) y los guarda como Ogg Vorbis.

Las escenas pueden tener sonido propio: capas `sound` con sonidos Ogg Vorbis, Opus, FLAC o WAV del
paquete (de 60 s como mucho cada uno), en bucle o de una vez, que empiezan y paran con una línea de
tiempo o una propiedad, con volumen, panorama y fundidos, mezcladas en un solo stream. Empiezan en
silencio, como los vídeos: suenan con `wprs-ctl unmute` o el botón de sonido de la app, de la
bandeja o de cada pantalla que muestra una escena con sonido, y siguen el volumen general y la
propiedad *Volumen* que aparece en los ajustes de cada escena con sonido. Solo suenan mientras la
escena se ve en una pantalla activa y sin tapar (se pausan y siguen con ella al taparla, aunque la
escena no se pause, al pausar o con batería, y se paran al quitarla), nunca con la pantalla
bloqueada, y una escena que se ve en varias pantallas suena en una sola. `playback.scene_sound = false`
(*Dejar que las escenas reproduzcan sus sonidos* en los ajustes de la app y en la bandeja, o
`wprs-ctl config playback.scene_sound false`) calla el sonido de todas las escenas sin tocar el de
los vídeos; el teléfono no reproduce el sonido de las escenas. Cada capa suena en el punto que marca
el tiempo de la escena: una capa que empieza con una línea de tiempo arranca en la posición de esa
línea (no desde cero), vuelve a empezar con cada vuelta de la línea, sigue sonando si la línea se
para y salta a donde está la línea al reanudarla; el daemon vuelve a alinear el sonido con la escena
cada segundo contando desde la posición que reproduce la salida de audio, más un ciclo de PipeWire
de 20 ms cuando el sonido va a PipeWire. Si la salida de audio falla o no
hay ninguna (también con el plugin de PipeWire viejo), la escena y los vídeos siguen sin sonido y lo
recuperan solos cuando aparece una salida. `install.sh` las instala en
`~/.local/share/wallpaper-rs/examples` (`--no-examples` las omite), el RPM en el paquete opcional
`wallpaper-rs-examples` y el Flatpak las incluye. Aparecen en la biblioteca y, si se quitan de
ella, no vuelven.

## Editor de escenas

La página *Escenas* de la app crea una escena a partir de un punto de partida (vacía, partículas,
paralaje por capas o reloj de texto), abre una carpeta de escena o edita una copia de una escena de
la biblioteca, en una carpeta nueva dentro de la de documentos (`wallpaper-rs/<título>`) o en la
que elijas. Una escena en edición es una carpeta con `manifest.json`, `scene.json` y sus recursos
en `assets/`: el editor no tiene un formato propio y cambia los mismos archivos que lee el motor.
Las escenas nuevas usan la segunda versión del manifiesto y nacen válidas: para todos los públicos,
sin destellos, obra original y con las entradas que ya usan declaradas, hasta que cambies esos
datos. `wprs-app --page scenes` abre esa página y, como con el editor de imágenes, una petición de
otra página no cierra un editor con cambios sin guardar.

- **Capas**: un árbol con todos los tipos de capa (imagen, vídeo, texto, forma, color, espectro,
  partículas, sonido, grupo y ajuste) para añadir, quitar, duplicar, subir y bajar, agrupar (Ctrl y
  clic eligen varias), meter en un grupo o sacar de él, ocultar (o silenciar un sonido), bloquear
  y dejar *solo* una capa en la vista previa. El asa de cada fila la arrastra a otro sitio de la
  lista, también dentro de otro grupo o fuera de él (la misma acción que `wprs-ctl scene
  move-layer`).
- **Lienzo**: la vista previa la dibuja el propio daemon en una salida virtual que llega por
  PipeWire, como el fondo de Plasma, así que necesita el módulo QML de KPipeWire. Ctrl y la rueda
  amplían, la rueda o un arrastre con el botón central o el derecho desplazan. Una capa sin
  bloquear se arrastra directamente, y la elegida gira y escala con sus asas; un arrastre que
  empieza fuera de las capas, o cualquiera con Mayúsculas, traza un rectángulo que elige las capas
  que toca, como Ctrl en la lista, listas para agruparlas. Las guías marcan lo que se ve en pantallas 16:9, 21:9, 32:9,
  4:3, 16:10 y 9:16. *Ver en el escritorio* lleva la escena sin guardar a todas las pantallas
  sin aplicarla; se quita al desactivarlo o al cerrar el editor, y como mucho un minuto después si
  la app se cierra de golpe.
- **Inspector**: se genera a partir del esquema JSON de las escenas (`wprs-ctl scene schema`), así
  que los campos nuevos del formato aparecen solos: números con deslizador y unidad, colores,
  opciones, vectores, curvas (un clic añade un punto, se arrastra y un doble clic lo quita),
  recursos, efectos con sus parámetros y la vuelta al valor por defecto de cada campo opcional. Los nombres de los campos y de sus valores (modos de fusión,
  formas, curvas de interpolación, efectos, tipos de capa y de recurso, idiomas, *Igual que la
  pantalla* de los fotogramas por segundo) salen en el idioma del escritorio, y una prueba recorre
  el inspector de cada punto de partida con todas sus variantes y falla si algo de lo que muestra
  no tiene traducción al español. Las capas y opciones nuevas que crea la app llevan nombres y
  textos en ese idioma. El rombo de cada valor animable pone o quita un fotograma clave en el
  instante actual o lo enlaza a una propiedad de usuario, al sonido o al puntero, y *Editar el
  origen* ajusta la línea de tiempo, la escala o el suavizado de ese enlace.
- **Línea de tiempo**: reproducir, pausar y mover el cabezal (la vista previa se queda en ese
  instante), una pista por valor animado con sus fotogramas clave, que se arrastran a otro
  instante, la interpolación de cada uno y la línea de tiempo que la mueve, y la lista de líneas de tiempo para crearlas y editarlas.
- **Paquete**: *Detalles del paquete* lleva el manifiesto al inspector, que también se genera a
  partir de su esquema (`wprs-ctl scene schema --manifest`): título, descripción y notas de esta
  versión en cada idioma (*Traducir* añade uno a partir del texto del idioma por defecto sin tocar
  los que ya hay, y los textos `@clave` se escriben en `strings`), versión, id, autores, licencia,
  clasificación (para todos o para adultos, con sus motivos), si destella, origen y obras de las que
  procede, créditos por archivo, miniatura y vista previa animada (*Elegir…* copia la imagen a
  `assets/` y reduce la miniatura si pasa de 1024 píxeles o de 1 MiB), enlaces, etiquetas, idioma
  por defecto y las entradas que puede pedir. Las entradas que usa la escena se marcan solas y no se
  pueden quitar, y `requires.features` se calcula en cada cambio como en `wprs-ctl pack build`:
  nunca se escribe a mano; las entradas que añadió el editor se van cuando nada las usa. Como
  `wprs-ctl pack check`, *Problemas* dice si el escritorio y el teléfono la rechazan o la reproducen
  sin alguna función. Una escena
  nueva toma como idioma por defecto el del escritorio. Un manifiesto de la primera versión ofrece
  *Actualizar*, que hace lo mismo que `wprs-ctl pack upgrade` (fija el idioma por defecto y cambia
  `velocidad-nieve` por `velocidad_nieve` también en la escena) pero no adivina la clasificación,
  los destellos ni el origen: quedan juntos en *Problemas* hasta que los eliges.
- **Propiedades y preajustes**: *Propiedades de usuario* las lista en el orden del formulario, cada
  grupo con sus miembros debajo, y sigue con los preajustes en la misma lista. Se añaden de
  cualquier tipo (deslizador, interruptor, color, selección, texto, archivo, carpeta, botón, grupo,
  etiqueta o separador) sin texto en ningún idioma, así que muestran su id hasta que les pones una
  etiqueta; se suben y bajan, se meten en un grupo o se sacan (los grupos no se anidan), se
  renombran (los enlaces, disparadores, grupos, condiciones y preajustes que las nombran cambian con
  ellas; los scripts no, y se avisa de los que la nombran) y se quitan junto con sus valores en los
  preajustes y en las condiciones de mapa, salvo mientras un enlace, un disparador o una expresión
  las nombra: entonces se dice dónde. El inspector edita cada campo de su tipo: paso, unidad,
  decimales, casilla para el número y escala logarítmica de un deslizador, valores, etiquetas e
  iconos de las opciones, color con transparencia, textos secretos, grupos plegados, archivos que
  acepta y recurso por defecto, carpetas con su orden y su intervalo, icono y orden, y muestra los
  textos `"@clave"` con los de `strings`. `visible_if` y `enabled_if` se escriben como una expresión
  del lenguaje de las condiciones o como un mapa JSON y se comprueban mientras escribes; solo se
  guarda una condición válida, y un problema de una condición o de un preajuste lleva a su campo.
  *Preajustes* lista los del paquete y *Añadir un preajuste* crea otro, con una fila por cada
  propiedad que pueden fijar. Solo las propiedades con valor se pueden enlazar, y el disparador de
  una línea de tiempo se elige entre `start`, `beat`, los botones y esas propiedades. Un valor que
  sigue a una selección o a un interruptor (la imagen de una capa, una máscara, la textura de unas
  partículas, un vídeo o un color) muestra la propiedad que lo guía y una fila por opción para
  elegir su variante. `requires.features` sigue a los tipos y campos nuevos que se usan.
- **Recursos**: importa imágenes, vídeos, sonidos, fuentes y tablas `.cube` dentro de los límites
  del formato y borra los archivos que la escena ya no usa. *Importar un efecto…* (`wprs-ctl scene
  import-effect`) copia un efecto propio, su descriptor y su shader WGSL (junto a él o elegido aparte,
  como `--wgsl`), a `effects/`, lo comprueba como al leer el paquete y dice qué cambiar si no pasa;
  después se añade a las capas como los integrados, con su nombre en tu idioma, y sus parámetros
  salen en el inspector con su etiqueta y su ayuda en tu idioma, sus límites, las imágenes o capas
  que pueden leer y sus variantes, enlazables a propiedades, fotogramas clave o scripts.
  *Importar un script…* (`wprs-ctl scene import-script`) copia un módulo `.js` a `scripts/` y
  propone su declaración: los eventos que exporta o atiende con `on` y las partes de `wprs` que
  importa o llama; conviene revisarla antes de publicar.
- **Problemas**: los mensajes del validador cambian con cada edición; al pulsar uno se elige la
  capa o el campo afectado. Esos mensajes y los rechazos del editor llevan un identificador con sus
  argumentos (`scene.color`, `editor.step.add_layer`…) y salen en el idioma del escritorio en la
  app y en `wprs-ctl`, y en el del teléfono cuando este muestra el mismo error; las capas de los
  puntos de partida (`sky`, `sun`, `flakes`…) y los pasos de deshacer se muestran traducidos sin
  cambiar los ids guardados. Un punto de partida creado en un idioma que sus etiquetas no tienen
  usa ese idioma, el del título, como idioma predeterminado y copia en él las etiquetas en inglés
  para traducirlas. Debajo, la consola de los scripts muestra las últimas 50 líneas que
  escribieron en la vista previa (`wprs-ctl state` las da en `console` de la salida `preview:`).

El editor de escenas pinta también las máscaras de los efectos que las piden por canal: junto a
cada parámetro de máscara hay un botón para pintar, el canal (rojo, verde, azul o alfa), un modo
para borrar y el tamaño del pincel, y cada trazo sobre la capa en la vista previa se guarda como una
imagen nueva en `masks/`, de modo que deshacer vuelve a la anterior (`wprs-ctl scene paint-mask`
hace lo mismo desde la terminal).

También recorta una malla de una imagen de la escena («Malla desde una imagen» en el menú para añadir
capas), con una cuadrícula de celdas o siguiendo el contorno de la parte opaca, y le pone huesos, igual
que a un `.glb` importado de varias partes. El inspector del campo `mesh` muestra los huesos en árbol:
un clic elige un hueso (también sobre la vista previa), un doble clic lo renombra (y la escena lo
sigue en clips, cadenas, física, reglas, puntero y capas sujetas), y un hueso soltado sobre otro
cuelga de él; al quitar un hueso o colgarlo de otro, en el mismo paso se quitan las claves, cadenas,
cuerpos de física, reglas, huesos de `hit` y capas sujetas que lo nombraban o que dejaron de
encadenarse, de modo que la escena sigue siendo válida. Sobre la vista previa los huesos se arrastran, se pinta la profundidad de los
vértices, que los acerca o aleja del espectador, y se pintan los pesos del hueso elegido con un
pincel que suma, resta, suaviza o normaliza, con fuerza, tamaño, borde suave y huesos bloqueados;
la malla se colorea por el peso de ese hueso o por el hueso que manda en cada parte, y cada vértice
queda con cuatro huesos como mucho y pesos que suman 1. Mientras no se pintan, los pesos siguen a
los huesos más cercanos; una vez pintados se conservan al cambiar los huesos, y *Pesos automáticos*
los vuelve a calcular. Cada cambio guarda una malla nueva en `meshes/`, de modo que deshacer vuelve a
la anterior; al guardar, las mallas anteriores esperan fuera de la carpeta, junto al historial del
editor, y vuelven cuando un paso las nombra, así que el paquete lleva solo las mallas en uso.

*Editar los clips en la línea de tiempo* cambia las líneas de tiempo bajo la vista previa por los
clips de la capa: se elige, crea, renombra o quita un clip, se fijan su duración y su bucle, y el
cabezal recorre, reproduce y repite la vista previa con ese clip, en segundos desde el comienzo del
clip (su `from`), donde se escriben también las claves. Hay filas por canal de cada hueso,
por el orden de las partes y por cada forma de mezcla, con un deslizador que guarda su peso; las
claves se arrastran (con Ctrl se copian), se borran y se les da paso, línea o curva suave. Sobre la
vista previa el hueso elegido tiene asas para moverlo, girarlo y escalarlo en ese instante, y
*Clave* guarda en las claves del clip del JSON de la escena, las mismas que reproduce el motor, solo
los canales que cambiaron las asas, o la pose entera del hueso (con el movimiento del archivo) si no
cambió ninguno. Las formas de mezcla tienen deslizadores que solo cambian la vista previa, y con ellos se
crean, cambian, renombran y quitan expresiones.

El mismo inspector tiene herramientas sobre la vista previa para el resto del personaje. *Crear una
cadena en la vista previa* pide un clic en el hueso donde empieza y otro en el hueso donde termina;
cada cadena elige qué alcanza (el puntero, un punto, un hueso o un punto de la malla, u otra capa),
se dobla hacia el otro lado, tiene peso y límites por hueso. El objetivo de punto es un anillo que se
arrastra y cada límite es un arco alrededor del hueso con dos puntos que fijan su ángulo mínimo y
máximo. Los huesos que se balancean se eligen con un clic sobre la vista previa, con su modo y
deslizadores de rigidez, amortiguación, inercia, gravedad (una flecha cuya punta se arrastra) y
viento, y casillas que acotan su giro (un arco con dos puntos que se arrastran) y cuánto se desliza
su cabeza;
*Dibujar una cuerda* crea, a lo largo del trazo, hasta 9 huesos nuevos encadenados que cuelgan del
hueso elegido y se balancean como cuerda, y *Simular* y *Reiniciar* hacen correr la física en la
vista previa o la devuelven a la pose de reposo. El viento de `canvas.wind` es una flecha que se
arrastra desde el centro del lienzo, con su ángulo, su fuerza y sus ráfagas (`canvas.gusts`). Para
sujetar capas, se arrastra una capa sobre un hueso, o sobre la malla para crear allí un punto nuevo
(un nodo sin malla del archivo, colgado del hueso que manda en ese lugar), eligiendo si sigue la
posición, el giro y la escala, y una línea une cada capa sujeta con su hueso o punto y acaba en un
anillo que se arrastra para moverla a lo largo de él. Los puntos de la malla se listan con su nombre,
que se edita ahí mismo, el hueso del que cuelgan y un botón para quitarlos, y sobre la vista previa
son cuadrados que se arrastran; un punto creado para una sola capa se va con ella cuando la capa
deja de seguirlo. Los puntos nunca cuentan como el final de un hueso, así que no cambian las
cadenas ni la física. Las partes se
ordenan arrastrándolas en una lista de atrás hacia delante, se ocultan y se recortan para verse solo
sobre otra parte, cuyo contorno se marca en la vista previa. En el inspector general, todo campo que
nombra un hueso, un punto, una parte o una forma de mezcla (cadenas, objetivos y límites, física,
reglas, máscaras de huesos de las capas de animación, `hit`, `attach` y `parts`) ofrece los de la
malla en una lista.

Desde la terminal hacen lo mismo `wprs-ctl scene mesh-from-picture`, `bone` (con `--rename`,
`--root` y `--remove`), `mesh-depth`, `weights` (pincel de pesos con `--mode`, `--by`, `--radius`,
`--falloff` y `--lock`), `auto-weights`, `clip` (con `--add`, `--remove`, `--rename`, `--length` y `--loop`), `key` (claves
de huesos, formas y orden, con `--ease`, `--remove`, `--from` y `--copy`), `expression`, `ik` (`add
RAÍZ PUNTA`, `set N` y `remove N`, con `--target`, `--limit HUESO MÍN MÁX`, `--bend` y `--weight`),
`physics` (`add HUESO...` con `--mode`, `--stiffness`, `--damping`, `--inertia`, `--gravity`,
`--wind`, `--rotation`, `--translation` y `--free`; `rope NOMBRE X,Y;X,Y;...` con `--parent`; `remove
HUESO...`), `wind` (`--angle`, `--strength`, `--noise` y `--off`), `attach` (`--to`, `--bone`,
`--point X,Y` o un nodo, `--offset`, `--nudge`, `--inherit` y `--detach`), `point` (puntos de sujeción
con `--at`, `--parent`, `--root`, `--rename` y `--remove`) y `parts` (`--order`, `--hide`,
`--show`, `--clip` y `--unclip`); `bones`, `vertex-weights` y `keys` imprimen los huesos y puntos,
los pesos de cada vértice y las claves de un clip.

Al elegir un sistema de partículas o una capa de partículas, la vista previa dibuja encima sus
herramientas: cada forma de emisor con asas (el punto, las esquinas de una caja, el centro y el radio
de un círculo, los extremos de una línea; para `around`, el centro, el radio y el final del arco, con
un punto en cada lugar de nacimiento; para `between`, sus extremos, un asa que curva la línea y otra
que desliza el punto más ancho del afinado; para `layer`, el contorno de la capa y los píxeles que
pasan el umbral), los puntos de control `p0` a `p7` como marcadores que se arrastran y un anillo con
un asa para la distancia focal de la perspectiva. El inspector del sistema ofrece el tipo de cada
emisor, a qué punto de control o al puntero siguen `around` y `between`, una lista de las capas desde
las que puede emitir (nunca una que contenga el sistema) con el umbral, su color, su movimiento y
cuándo se vuelve a leer, los puntos de control con su dial de ángulo, las reasignaciones de valores
iniciales con sus entradas y salidas, rangos de dos asas, límites, operación, onda y la curva de la
salida frente a la entrada (un componente de curvas que también pueden usar otras herramientas), el
recorte de alfa con su rango, su opacidad y la rampa que da, la orientación con un dial, la
perspectiva con su distancia focal y los rangos de profundidad, lo que cada hijo toma de su
partícula padre y los shaders de partículas del paquete con el aviso de que el motor los comprueba y
puede dejarlos fuera. Desde la terminal hace lo mismo `wprs-ctl scene particles CARPETA SISTEMA` con
`shape`, `control-point`, `remap add|set|remove`, `cutout`, `orient`, `perspective`,
`emit-from-layer`, `inherit` y `shader`, y `handles` imprime lo que la vista previa dibuja.

Cada cambio es una operación sobre un JSON Pointer. Deshacer y rehacer (Ctrl+Z y Ctrl+Mayús+Z)
guardan hasta 500 pasos o 64 MiB, y un arrastre cuenta como un solo paso. Lo que no se ha guardado
se copia cada 30 segundos y al cerrar en `~/.local/state/wallpaper-rs/editor/`, que solo puede leer
tu usuario, y la app ofrece recuperarlo al volver a abrir la carpeta; la copia se borra al cerrar
con todo guardado y las que llevan 30 días sin usarse se borran solas. Importar, compilar y aplicar
trabajan en segundo plano, sin bloquear la ventana. *Compilar y aplicar…* crea el `.wpak` con el
mismo código que `wprs-ctl pack build`, lo firma si eliges una clave de autor (como `wprs-ctl pack
sign`) y lo aplica a una pantalla o a todas.

Cada acción del editor tiene su orden en `wprs-ctl scene`, que trabaja sobre la carpeta sin daemon
(salvo `preview` y `apply`, que usan el servicio), guarda al momento y comparte con la app el
historial de deshacer, donde cada orden es un paso (todas en
[`docs/specs/editor.md`](docs/specs/editor.md)):

```sh
wprs-ctl scene new mar --from parallax --title Sea --locale en   # empty, particles, parallax o clock
wprs-ctl scene open copia --from escena.wpak            # una copia sin firma, derivada, con el id <id>.copy
wprs-ctl scene import mar ola.ogg                       # copiar un recurso a assets/
wprs-ctl scene add-layer mar sound --source ola         # y remove-layer, duplicate-layer, move-layer
wprs-ctl scene add-layer mar text --name Título         # id «titulo», con ese texto
wprs-ctl scene set mar /scene/layers/1/opacity 0.5      # cualquier campo por su JSON Pointer
wprs-ctl scene keyframe mar /scene/layers/1/opacity 2 1 --ease in_out_sine
wprs-ctl scene keyframe mar /scene/layers/1/opacity 3 --from 2   # moverlo a otro instante
wprs-ctl scene translate mar /manifest/title es Mar     # otro idioma; --add no pisa nada, --remove lo quita
wprs-ctl scene set mar /manifest/content '{"rating": "mature", "descriptors": ["violence"]}'
wprs-ctl scene thumbnail mar captura.png                # miniatura en assets/, reducida si hace falta
wprs-ctl scene upgrade vieja --rating general --photosensitive false   # como pack upgrade, con deshacer
wprs-ctl scene add-property mar combo --id tiempo      # y move-property, rename-property, remove-property
wprs-ctl scene set mar /manifest/properties/tiempo/options '["calma", "tormenta"]'
wprs-ctl scene set mar /manifest/properties/tiempo/default calma
wprs-ctl scene add-property mar slider --id velocidad --label Speed
wprs-ctl scene translate mar /manifest/properties/velocidad/label es Velocidad
wprs-ctl scene condition mar velocidad "tiempo == 'tormenta'" --enabled   # comprobada como en validate
wprs-ctl scene add-preset mar --label Calm && wprs-ctl scene set mar /manifest/presets/0/values/tiempo calma
wprs-ctl scene group mar near far --id colinas && wprs-ctl scene hide mar sky
wprs-ctl scene undo mar && wprs-ctl scene history mar
wprs-ctl scene validate mar                             # problemas, cada uno con su JSON Pointer
wprs-ctl scene preview mar -o DP-1 --time 2             # verla en el escritorio 60 s sin aplicarla
wprs-ctl scene build mar --key autor.key                # firmar y crear el .wpak
wprs-ctl scene apply mar -o DP-1                        # añadirla a la biblioteca y mostrarla
```

### Guía para autores de scripts

Un script es un módulo JavaScript del paquete que la escena nombra en `scripts` (como mucho 64 y 1 MiB
entre todos). En el ordenador todos los de una escena comparten un proceso aislado, y en el teléfono
un hilo, también cuando varias pantallas
muestran la misma escena con los mismos ajustes, con 32 MiB de memoria y 4 ms de CPU por fotograma
para todos juntos (100 ms para cargar y para el primer fotograma). Lo que devuelven llega al
fotograma siguiente, así que un script lento nunca frena el dibujo, y solo viaja lo que cambió:
cada campo que un script escribe se compara con el que ya tenía, sin pasar la salida entera a texto
en cada fotograma, y un fotograma sin cambios no envía nada. `docs/specs/scene.md` describe la
interfaz completa; lo esencial:

- **Ciclo de vida**: el módulo exporta las funciones que necesita: `init(ctx)` en el primer
  fotograma, `update(frame)` en cada uno, `destroy()` al parar, `resize({screen, canvas})` al cambiar
  o girar la pantalla, `properties(changed)` con los ajustes que cambian (todos al
  empezar), `settings({language, fps, quality})` y `timelineEvent(e)`. Sin `update` el script solo
  despierta por eventos y la escena no dibuja fotogramas de más.
- **Un valor, una función**: `{"script": "scripts/olas.js", "fn": "vaiven", "params": {"velocidad":
  2}}` llama cada fotograma a `vaiven(valor, ctx)` con lo que devolvió la vez anterior y `ctx.path`,
  `ctx.layer` y `ctx.params`; un mismo módulo sirve así a muchas propiedades. Los `params` se declaran
  solo en la entrada del script en `scene.json`, con tipo, valor por defecto y límites: el módulo no
  los exporta y `wprs-ctl scene import-script` no los propone, así que se escriben a mano.
- **Capas**: `scene.layer(id)` lee la posición, la escala, el giro, la opacidad, el color, el tamaño,
  el texto y lo demás tal como la escena los evaluó, y escribe esos campos, que duran hasta que el
  script los cambie o llame a `reset()`. `layer.effect("nombre")` apaga un efecto, cambia sus
  parámetros dentro de su rango o vacía lo que guarda. Con plantillas (`templates`),
  `scene.spawn("barra", {position: [x, y]})` crea hasta 256 copias, que con las capas de la escena y
  las de dentro de cada copia suman como mucho 512 (una plantilla tiene como mucho 256), `destroy()`
  las quita y `scene.order(id, índice)` las reordena: así se hacen los visualizadores.
- **Datos de entrada**: `audio.buffers(32)` devuelve las bandas izquierda, derecha y media en
  `Float32Array` que el motor rellena solas en cada fotograma; `input.cursor` da la posición en
  fracciones, en píxeles del lienzo y de la pantalla y el botón izquierdo; `engine` da el tamaño del
  lienzo y de la pantalla, la orientación, el idioma, la calidad, los fotogramas por segundo y si es
  fondo, pantalla de bloqueo o editor. El sonido y el puntero siguen necesitando su permiso.
- **Puntero y clics**: una capa con `"solid": true` (o `{"alpha": 0.5}` para contar solo donde es
  bastante opaca) recibe el puntero. Sin scripts, una línea de tiempo con `"trigger": "click:boton"`,
  `"enter:boton"` o `"leave:boton"` arranca al hacer clic, entrar o salir, y un sonido con ese
  `play_on` suena. Con scripts, `scene.layer("boton").on("click", (e) => …)` recibe `{type, layer,
  local, scene}` para `click`, `down`, `up`, `enter`, `leave` y `move`; la capa que recibe `down` se
  queda el puntero hasta soltar, aunque salga de ella, así que arrastrar es seguir `move` entre `down`
  y `up`, y un clic solo cuenta si se suelta sobre la misma capa. Se declara en `events` y la escena
  pide las entradas `pointer` y `clicks`: el usuario las permite por paquete y, los clics, también
  por pantalla, donde empiezan apagados. En el teléfono un toque es un clic.
- **Padres, cámara y ajustes de escena**: con `uses: ["hierarchy"]`, `layer.parent`,
  `layer.children` y `layer.world` (una `Mat3`) dicen a quién sigue una capa, quién la sigue y dónde
  queda en el lienzo, y `layer.setParent("sol", true)` la hace seguir a otra capa sin moverla ni
  cambiar el orden de dibujo; `lookAt(desde, hacia)` de `wprs/math` da el ángulo para mirar hacia un
  punto. Con `uses: ["camera"]`, `scene.camera = {zoom: 2, shake: {amplitude: 6}}`,
  `scene.parallax`, `scene.bloom` y `scene.background` cambian la cámara, el paralaje, el resplandor
  y el color de fondo de la escena, y `null` los devuelve a lo que dice `scene.json`.
- **Formas propias**: con `uses: ["geometry"]` y un `pools` en `scene.json`,
  `scene.mesh("barras").create({vertices, indices, format: "xyuv"})` crea triángulos con
  `Float32Array` que `update()` cambia y `destroy()` quita, dentro de los límites del pool y de 1 MiB
  por fotograma. Se dibujan en la imagen del pool, que una capa `video` muestra con `"video":
  "barras"`; un pool que no cambia no pide fotogramas.
- **Guardar datos**: con `uses: ["storage"]`, `storage.set("visitas", n)` y `storage.get` guardan
  JSON del paquete o, con `"output"`, de la pantalla: como mucho 64 KiB cada uno, solo en el
  dispositivo (sin sincronizar), en un archivo privado que se escribe como mucho cada 5 s y al parar
  y se borra al quitar o desinstalar el paquete. Las pantallas que muestran el paquete comparten lo
  del paquete (lo que guarda una llega a las demás en su siguiente fotograma) y las vistas previas
  del editor lo leen sin escribirlo.
- **Carpeta elegida**: con `uses: ["folders"]` y una propiedad `folder` con `"mode": "all"`,
  `folder("fotos").files` da identificadores opacos de sus imágenes, `name(id)` el nombre del
  archivo, `on("added" | "removed" | "changed", …)` avisa de los cambios y `show("marco", id)` elige
  qué foto muestra una capa. La carpeta se mira cada 2 s fuera del hilo que dibuja, también con la
  escena quieta. Los scripts nunca ven rutas.
- **Sonido**: con `uses: ["sounds"]`, `sound("campana")` controla una capa de sonido con `play()`,
  `pause()`, `stop()`, `playing` y `volume`, de 0 a 1 y por debajo del volumen general, que también
  la silencia. La primera llamada se queda el sonido aunque tuviera `play_on`. El teléfono no
  reproduce sonidos de escena, así que allí esas llamadas no hacen nada.
- **Tiempo y depuración**: `setTimeout` y `setInterval` siguen el reloj de la escena (como mucho 64,
  una vez por fotograma y nunca por debajo de 16 ms) y devuelven una función para cancelarlos;
  `console.log` y sus hermanas escriben en el registro y en el editor, como mucho 20 líneas por
  segundo. `shared` guarda valores comunes a todos los módulos (una copia congelada de lo que se
  asigna: para cambiarlo se asigna otra vez), que también se importan entre sí con rutas relativas
  (`import { media } from "./lib/media.js"`).
- **Matemáticas**: `wprs/math` trae `Vec2`, `Vec3`, `Vec4`, `Mat3` y `Mat4` con sus operaciones
  (producto, inversa, descomposición, reflejo, proyección…), `smoothstep`, `mix`, `remap`, ángulos,
  `lookAt`, `rgbToHsv` y `hsvToRgb`.
- **Declarar lo que se usa**: cada entrada de `scripts` lista sus `events` y sus `uses`; un módulo
  que declara algo solo puede usar los temporizadores, las capas creadas y los efectos si los
  declara, y `pack build` deduce de ahí las funciones nuevas que pide el paquete, para que un motor
  más viejo avise en vez de ejecutarlo mal. Un módulo que no declara nada recibe la primera
  interfaz, igual en todos los motores: `update(frame)` con un `frame` nuevo cada vez, `engine`,
  `time`, `props`, `audio`, `input`, `scene.layer`, `timeline` y `particles`; si importa algo más,
  exporta `init` u otras funciones del ciclo de vida o usa `console` o los temporizadores, `pack
  build`, `pack check` y el editor lo rechazan y el motor no se lo da.
- **Errores**: una función que lanza un error lo escribe en la consola y el resto sigue; un módulo
  que falla tres fotogramas seguidos se detiene solo, con sus temporizadores, y la escena sigue sin
  él con un aviso traducido en la app, la bandeja y `wprs-ctl status`. Si los scripts agotan su
  tiempo o su memoria tres fotogramas seguidos, se apagan y la escena sigue sin ellos.

```js
import { scene, audio, sound, console } from "wprs";
import { smoothstep } from "wprs/math";

const bandas = audio.buffers(32);
const barras = [];

export function init() {
  for (let i = 0; i < 32; i++) barras.push(scene.spawn("barra", { position: [40 + i * 60, 900] }));
  scene.layer("tambor").on("click", () => sound("golpe").play());
  console.log("barras listas");
}

export function update() {
  barras.forEach((b, i) => (b.scale = [1, 0.1 + smoothstep(0, 1, bandas.average[i]) * 4]));
}
```

## Desarrollo

Dependencias de compilación (Debian/Ubuntu, con GStreamer 1.24 o posterior, como en Ubuntu 24.04):
`libpipewire-0.3-dev libclang-dev pkg-config libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev
qt6-base-dev qt6-declarative-dev`. Para las
pruebas: `mesa-vulkan-drivers pipewire wireplumber dbus-x11 gnome-keyring gstreamer1.0-plugins-base
gstreamer1.0-plugins-good gstreamer1.0-pipewire gstreamer1.0-tools`, `sway grim foot` para la de
Sway y `xvfb x11-utils x11-xserver-utils xdotool imagemagick openbox` para la de X11.

```sh
cargo test --workspace                   # pruebas unitarias, de render y de vídeo
cargo build -p wprs-pipewire --example probe
cargo build -p wprsd -p wprs-ctl && cargo build -p wprsd --example timedate
cp target/debug/examples/probe target/debug/
scripts/integration-test.sh target/debug # D-Bus + PipeWire + daemon de punta a punta
scripts/sway-test.sh target/debug        # superficies layer-shell en un Sway sin pantalla
scripts/x11-test.sh target/debug         # ventanas de escritorio en un Xvfb
cargo build -p wprs-server && cargo build -p wprs-android --example phone
cp target/debug/examples/phone target/debug/
scripts/sync-test.sh target/debug        # daemons, el cliente del teléfono y wprs-server: enlace, clave y sincronización
scripts/package.sh                       # genera dist/wallpaper-rs-*.tar.gz
scripts/translations.sh update [idioma]  # actualiza po/ con los textos del QML, del validador y del editor
target/debug/wprs-app --check            # carga todo el QML de la app y falla ante cualquier aviso
```

`wprs-app --check` recorre cada página con datos de prueba, abre los detalles de un elemento, el
editor de imágenes y el de escenas, y falla ante cualquier aviso del QML o si no termina en 240 s.
Los archivos QML siguen el formato de `qmlformat` de Qt 6.12.

APK de Android (con el SDK, el NDK 28 o posterior y `cargo install cargo-ndk`):

```sh
rustup target add aarch64-linux-android x86_64-linux-android
ANDROID_NDK_HOME=/ruta/al/ndk android/gradlew -p android assembleDebug
adb install -r -d android/app/build/outputs/apk/debug/app-arm64-v8a-debug.apk
android/gradlew -p android connectedDebugAndroidTest -Pabis=x86_64  # pruebas en un emulador x86_64
scripts/android-power.sh -o consumo.csv  # consumo, fps y memoria por escenario en el teléfono
ANDROID_NDK_HOME=/ruta/al/ndk scripts/apk-check.sh android/app/build/outputs/apk/release/app-universal-release.apk
WANT="arm64-v8a " scripts/apk-check.sh android/app/build/outputs/apk/release/app-arm64-v8a-release.apk
```

Gradle hace un APK por arquitectura (`app-arm64-v8a-…`, `app-x86_64-…`) y, con más de una, el
universal (`app-universal-…`); el de release pasa por R8, que quita el código de las bibliotecas
que no se usa y deja entero el de la app, que Rust llama por JNI. `scripts/apk-check.sh`, que la
CI pasa a los tres APK publicados, comprueba que cada uno trae justo las arquitecturas que debe con
su biblioteca de Rust, que no trae ONNX Runtime, que todas las bibliotecas están alineadas a
páginas de 16 KB, que el APK lo está para `zipalign` y que la biblioteca de Rust no guarda rutas
del ordenador que la compiló.

`scripts/android-power.sh` recorre por adb los escenarios de consumo (imagen fija, desplazamiento,
fondo tapado, pantalla apagada, ahorro de energía, temperatura alta, paralaje con el teléfono quieto
y vídeo, 30 minutos y 3 veces cada uno) y añade una fila por medida al CSV. Con `trace_processor` de Perfetto en el `PATH` usa los
raíles de energía del teléfono; si no, la estimación de batterystats. Conviene conectar adb por
Wi-Fi para que el cable no cargue el teléfono durante la medida.

La CI firma el APK con la clave de alias `wallpaper-rs` de un almacén PKCS12 guardado en base64 en
el secreto `ANDROID_KEYSTORE`, con su contraseña en `ANDROID_KEYSTORE_PASSWORD`; sin ellos usa una
clave temporal y lo avisa. Se crea una vez con:

```sh
keytool -genkeypair -keystore wprs.keystore -storetype PKCS12 -alias wallpaper-rs \
  -keyalg RSA -keysize 4096 -validity 36500 -dname CN=wallpaper-rs
base64 -w0 wprs.keystore
```

Una compilación local usa ese almacén con `WPRS_KEYSTORE=/ruta/al/almacen` y
`WPRS_KEYSTORE_PASSWORD`; si no, se firma con la clave de depuración del ordenador y antes de
instalarla hay que quitar el APK publicado con `adb uninstall io.github.wallpaper_rs`, que borra
también la imagen elegida. Su número de versión es 1 y el de la CI es el de cada ejecución, de
ahí el `-d` de `adb install`.

Flatpak local (compila sin red sobre el SDK de KDE 6.11, con Rust 1.99.1 de static.rust-lang.org y
las fuentes de Cargo descargadas antes por `flatpak-builder`, comprobadas con su SHA-256). Solo
entran los archivos que sigue git, con sus cambios sin confirmar; los que no sigue, como otras copias
de trabajo dentro de la carpeta, se quedan fuera. El SDK y la extensión de LLVM vienen de Flathub:

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
packaging/flatpak/sources.sh            # copia a target/flatpak-src y genera cargo-sources.json
flatpak-builder --user --install --install-deps-from=flathub --force-clean \
  --state-dir=target/flatpak-builder target/flatpak \
  packaging/flatpak/io.github.wallpaper_rs.WallpaperRs.yml
```

Con `WPRS_VERSION`, `sources.sh` pone esa versión en la copia (sin ella, la del `Cargo.toml`) y
`packaging/flatpak/releases.sh` escribe en el metainfo de la copia la lista `<releases>`: esa versión
con la fecha del último commit y las novedades y arreglos desde la etiqueta anterior, y las diez
versiones publicadas antes, así que `appstreamcli` ya no avisa de `releases-info-missing`. La CI construye el Flatpak para
x86_64 y para aarch64, este en un runner ARM de GitHub (gratuito en los repositorios públicos), y
`packaging/flatpak/check.sh` instala cada paquete y comprueba las versiones, `wprs-app --check` sin
pantalla, los archivos exportados (lanzador, icono, metainfo y servicios D-Bus), las órdenes de
`wprs-plasma` y que una edición hecha dentro del Flatpak llega a `~/.local/share/wallpaper-rs/edits/`.
Instala en el sistema y toca esa carpeta, así que fuera de la CI conviene usarlo en un contenedor
desechable.

Todas las acciones de GitHub están fijadas por el SHA completo de su commit, las imágenes de
contenedor por su resumen y las herramientas que descarga la CI (`scripts/tools.sh`: actionlint,
cosign y syft) por versión y SHA-256. La CI pasa actionlint, con shellcheck, por todos los
workflows. Al publicar una versión, `scripts/release.sh` genera con syft el SBOM de los archivos que
sigue git y firma `SHA256SUMS` con cosign sin clave propia (OIDC de GitHub; el trabajo solo tiene
permiso de escritura en el contenido y de pedir el token), y `scripts/server-image.sh` construye
la imagen del servidor con un BuildKit fijado, le adjunta los SBOM y la procedencia, la sube a
GHCR, firma su resumen y le añade la atestación con el SBOM de las fuentes. La primera imagen crea
el paquete `wprs-server` como privado: hay que hacerlo público una vez en sus ajustes de GitHub.

Los fondos web se prueban donde hay WPE WebKit: el trabajo *Web wallpapers* de la CI ejecuta,
en un contenedor de Arch con `wpewebkit`, la prueba del auxiliar (`apps/wprs-web/tests/page.rs`:
la interfaz `window.wprs`, el límite de FPS, la pausa, que la página no llega a la red, a otros
paquetes ni a archivos del equipo) y `scripts/web-test.sh` con el servicio, `wprs-ctl` y una
página de color. En Ubuntu, `scripts/integration-test.sh` comprueba que el fondo se rechaza con su
motivo. `WPRS_REQUIRE_WEB=1` hace que ambas fallen si falta el motor. En el teléfono,
`WebPageTest` (pruebas instrumentadas en el emulador) muestra una página de prueba con el WebView y
comprueba sus valores, el color que dibuja, la pausa, el límite de fotogramas, lo que oculta cada
ajuste de privacidad, el borrado de sus datos, los archivos elegidos, los clics solo con permiso y
que no llega a la red, a otros paquetes ni a archivos del teléfono.

La CI ejecuta además `scripts/plasma-test.sh` en un contenedor con Plasma 6: instala el
paquete con `install.sh`, arranca una sesión Wayland sin pantalla (KWin y plasmashell sobre
dispositivos DRM virtuales), activa el plugin, comprueba los píxeles de una captura, carga el icono
de la bandeja, abre la app en su página *Ajustes* desde el botón de la bandeja y con
`--page settings`, comprueba en un bus aparte que la bandeja solo ofrece la app donde está
instalada y, con `qmltestrunner` de `qt6-declarative` (`tests/tray-row.qml`), que la miniatura de
una pantalla, en la bandeja y en la app, sigue difuminada con un paquete para adultos hasta
pulsarla con el ratón o el teclado, y que la bandeja avisa si el fondo destella. También ejecuta
`scripts/sway-test.sh` en un Sway sin pantalla: captura cada salida con `grim`, conecta y quita una
pantalla y comprueba que el vídeo se pausa bajo una ventana a pantalla completa y con la pantalla
apagada, y `scripts/x11-test.sh` en un Xvfb con dos monitores, el fondo de la ventana raíz y la
misma pausa.

| Ruta | Contenido |
|------|-----------|
| `crates/wprs-core` | Configuración, modos de ajuste, nombres D-Bus |
| `crates/wprs-render` | Renderizador wgpu, sin ventana o sobre superficies Wayland, X11 y Android, con escenas y conversión de vídeo YUV |
| `crates/wprs-depth` | Mapas de profundidad: modelo bajo demanda, cálculo con ONNX Runtime y caché |
| `crates/wprs-scene` | Modelo de las escenas: capas, valores enlazados, líneas de tiempo y efectos, sin GPU, y su esquema JSON |
| `crates/wprs-editor` | Editor de escenas compartido por la app y `wprs-ctl scene`: operaciones, historial, puntos de partida, inspector y carpetas de escena |
| `crates/wprs-pipewire` | Exportación de streams de vídeo PipeWire |
| `crates/wprs-video` | Reproducción de vídeo en bucle y del sonido de las escenas con GStreamer |
| `crates/wprs-wayland` | Superficies layer-shell y ventanas de escritorio X11, escala, ritmo y oclusión |
| `crates/wprs-audio` | Captura del sonido con PipeWire y su análisis en bandas para las escenas, también a partir del espectro que da Android |
| `crates/wprs-script` | Scripts de escena en JavaScript, aislados en un proceso por escena con límites de CPU y memoria, su módulo `wprs` y `wprs/math` |
| `crates/wprs-sync` | Cifrado, relojes, fusión, protocolo, buzón y formato de los registros de la sincronización, compartidos por el escritorio y Android |
| `crates/wprs-android` | Fondo animado de Android en Rust: JNI, GPU compartida, una superficie por fondo, biblioteca, listas, ajustes, escenas, paralaje, mapas de profundidad, repositorios y sincronización con el servidor propio (archivos privados, frase de recuperación, avisos al instante y certificados fijados) |
| `apps/wprsd` | Daemon |
| `apps/wprs-ctl` | CLI |
| `apps/wprs-app` | App de escritorio (Qt Quick y Kirigami sobre Rust con cxx-qt) |
| `apps/wprs-server` | Servidor propio: repositorio, catálogo, subidas, moderación y sincronización |
| `android` | App de Android: proyecto Gradle, servicio de fondo, reproductor de vídeo e interfaz Compose en Kotlin |
| `integrations/plasma-wallpaper` | Plugin de fondo de Plasma 6 |
| `integrations/plasma-tray` | Icono de gestión en la bandeja de Plasma 6 |
| `integrations/kwin-pointer` | Script de KWin que envía la posición del puntero mientras una escena o un paralaje la usa |
| `po` | Traducciones gettext de los paquetes de Plasma y de la app |
| `packaging` | Servicio systemd, activación D-Bus, lanzador e icono, scripts de instalación, Flatpak |
| `deploy` | Despliegue de referencia del servidor con Caddy y con la imagen publicada |
| `docs` | Especificaciones de la interfaz D-Bus, los paquetes y sus escenas, el editor de escenas, los repositorios firmados y el servidor |

## Privacidad

- Sin telemetría ni estadísticas de uso. El servicio solo se conecta a internet para los
  proveedores de fondos, los repositorios y el servidor propio que añadas tú. Al arrancar solo contacta con un
  proveedor si le toca su sincronización automática; la comprobación diaria de los repositorios
  (`repos.auto_refresh`) empieza 10 minutos después. Un repositorio solo puede redirigir a su propia
  dirección y a las réplicas que declara en su raíz firmada.
- Los proveedores de fondos en línea (Bing, APOD, Wikimedia, Wallhaven, WebDAV/Nextcloud, Immich)
  solo se usan si los configuras. Los servicios públicos se usan siempre por HTTPS; todos llevan un
  User-Agent genérico (`wallpaper-rs/<versión>`), sin cookies ni cabecera Referer, y las claves de API
  y contraseñas solo se envían al servidor configurado. Se guardan en el llavero del sistema
  (Secret Service) y en ningún otro sitio: ni en la configuración, ni en los registros, ni en la
  línea de órdenes. `http://` sin cifrar a otro equipo solo se permite con `allow_plain_http`.
- Los fondos web no tienen red salvo que la declaren y la permitas; lo que ven de ti (zona horaria,
  idioma, tarjeta gráfica y sus datos guardados) se avisa en la app, la bandeja, `wprs-ctl` y el
  teléfono, y puedes ocultarlo por fondo. En el teléfono el WebView va con sus métricas y la
  navegación segura apagadas.
- El modelo de profundidad y ONNX Runtime solo se descargan si lo pides y aceptas su tamaño,
  licencia y servidores; llevan el mismo User-Agent genérico, sin cabecera Referer ni redirecciones
  de HTTPS a HTTP, se verifican con SHA-256 y los mapas se calculan en el equipo, en un proceso
  aparte. ONNX Runtime funciona con su telemetría desactivada: no guarda ningún identificador ni se
  conecta a nadie. En Android, ONNX Runtime se descarga junto con el modelo y solo después de que lo aceptes,
  comprobado con su SHA-256, y se usa con la telemetría apagada.
  `models.allow_download` se queda en cada equipo y, si la configuración no se puede leer, cuenta
  como desactivado.
- La configuración se escribe con permisos `0600` dentro de una carpeta `0700`: solo tu usuario
  puede leerla.
- La caché de descargas se guarda en una carpeta privada.
- La biblioteca y las copias importadas (`~/.local/share/wallpaper-rs`), las miniaturas y las
  entradas de paquetes (`~/.cache/wallpaper-rs`) se guardan con permisos `0600` en carpetas `0700`.
  Las miniaturas y el análisis de la biblioteca (tamaño, duración, color) los hace un proceso aparte
  con la prioridad más baja de CPU y disco. La vista del editor de imágenes se guarda como un
  archivo `0600` en la carpeta de ejecución de tu sesión (`XDG_RUNTIME_DIR`, o la carpeta temporal
  si no la hay) y se borra al dibujar la siguiente o al cerrar el editor.
- El servicio de systemd solo puede abrir conexiones de red (IPv4 e IPv6) para los proveedores y
  repositorios que añadas tú, y sus registros no incluyen rutas, nombres de archivo, servidores
  propios ni el modelo de GPU salvo con `RUST_LOG=debug`.
- Ninguna escena oye el sonido, sigue el puntero ni recibe clics sin tu permiso, que se da y se
  retira por escena desde la app, la bandeja o `wprs-ctl input grant|revoke <escena>
  audio|pointer|clicks` mientras la escena se ve. Los clics, además, están apagados en todas las
  pantallas hasta que los enciendes en una (`wprs-ctl input clicks on -o <pantalla>`, o en la app), y
  solo llegan los reales: los del compositor en Sway y demás escritorios con `wlr-layer-shell`
  (también los toques de una pantalla táctil), los de X11 que no manda otro programa con `SendEvent`
  y, en Plasma, los que el escritorio deja al fondo, que el plugin de fondo envía y el daemon solo
  acepta del intérprete de Plasma (ni de la app ni de `wprs-ctl`) y, mientras el script de KWin
  informa del puntero, solo en la pantalla donde está. Ningún otro programa puede inyectar clics por
  D-Bus; el daemon no distingue el plugin de otro código que corra dentro de Plasma, como otro
  applet, y por eso lo acota en vez de impedirlo. El permiso queda atado al paquete que se veía: a la clave que firma un paquete
  instalado desde un repositorio (sus actualizaciones lo conservan) o al contenido exacto de
  cualquier otro (en la vista previa del editor, a la carpeta del borrador si la pide la app o
  `wprs-ctl`, o a su contenido exacto si la pide otro programa), así que otro paquete con el mismo id no lo hereda, ni siquiera si se ve a la vez en
  otra pantalla (entonces `wprs-ctl input grant` pide `-o` con la pantalla, y la app y la bandeja
  dan el permiso al paquete de la pantalla que preguntan); los permisos guardados por versiones
  anteriores, que solo tenían el id, se vuelven a preguntar. Si otro programa lo pide por D-Bus, el
  daemon no lo da: te pregunta con una notificación del escritorio de wallpaper-rs que nombra la
  escena, solo cuenta tu respuesta en ella y solo la aplica a ese paquete (*Ahora no* tampoco quita
  el permiso a otros paquetes con el mismo id); si respondes *Ahora no*, la cierras o no contestas,
  esa pregunta no vuelve hasta diez minutos después, y la app, la bandeja y `wprs-ctl input status`
  lo indican; pasado ese rato, un permiso que rechazaste se puede volver a pedir igual. Pedir un
  permiso que ya está dado no pregunta nada. Tampoco acepta posiciones del puntero de nadie salvo del
  script de KWin que él mismo carga (fuera del Flatpak, solo si lo envía un `kwin_wayland` o
  `kwin_x11` del sistema) y de `wprs-ctl`. Cuando se aplica
  una escena que los pide sin permiso, desde donde sea (la app, la bandeja, una lista o `wprs-ctl`),
  la app y la bandeja preguntan una vez por escena qué permitir, explicando qué se capta y que nada
  se guarda ni se envía, y sus ajustes muestran un aviso con el botón *Permitir* hasta que decides;
  *Ahora no* se recuerda y no vuelve a preguntar. `wprs-ctl lib apply`, `playlist start`, `next` y
  `prev` imprimen la orden exacta para darlo. Aun con permiso, la captura del sonido (un stream de
  PipeWire llamado «wallpaper-rs audio analysis», que puede encender el indicador de micrófono) y el
  seguimiento del puntero solo funcionan mientras se ve una escena que los usa, y se paran al pausar,
  tapar o bloquear la pantalla, aunque la pausa al tapar esté desactivada, y cuando nadie mira el
  stream de esa pantalla (en Plasma, cuando el escritorio no se ve). Si PipeWire se reinicia, la
  captura vuelve a conectarse sola, primero a los 0,5 s y luego esperando el doble cada vez, hasta
  30 s, sin despertar al equipo mientras está conectada. `playback.scene_capture = false`
  (*Dejar que las escenas reaccionen al sonido que suena* en los ajustes de la app, la bandeja y el
  teléfono, o `wprs-ctl config playback.scene_capture false`) bloquea la captura para todas las
  escenas aunque tengan permiso. Del sonido solo se
  calculan bandas y niveles en memoria: no se guarda, no se registra y no sale del equipo. Del
  puntero solo se guarda en memoria la última
  posición, de los clics solo los que esperan al siguiente fotograma, y nunca se leen las teclas. Una imagen cuyo paralaje sigue al puntero lo usa con las
  mismas reglas, porque elegir esa fuente es el permiso. En Android, el permiso del micrófono se pide
  la primera vez que se activa *Reaccionar al sonido* en una escena, y solo se escucha mientras se ve
  una escena que lo usa y tiene ese permiso.
- Los scripts de las escenas se ejecutan en un proceso aparte por escena, sin archivos, red ni
  otros procesos, con límites de memoria, de CPU por fotograma, de temporizadores, de capas creadas,
  de líneas de consola y de valores compartidos, y nunca ven rutas, nombres de pantallas ni la
  batería.
- Con un servidor propio, los registros sincronizados salen cifrados con una clave que solo
  tienen tus equipos (en Android, guardada cifrada con una clave del almacén de claves del
  teléfono, que no sale de él); los identificadores de los paquetes se derivan de esa clave y el nombre de
  cada equipo solo viaja cifrado. La clave pasa a un equipo nuevo por el buzón del servidor,
  sellada para él, después de comparar un código de 6 cifras en las dos pantallas.
- Los paquetes se generan sin rutas, usuarios ni nombre del equipo de quien los compila.
- El servidor propio no guarda IP ni user agents en sus registros salvo que su administrador
  active el registro de abusos, que se borra solo; los datos sincronizados le llegan cifrados.

## Licencia

GPL-3.0-or-later. Ver [`LICENSE`](LICENSE). Las escenas de ejemplo de `scenes/` son de dominio
público (CC0 1.0, [`scenes/LICENSE`](scenes/LICENSE)).
