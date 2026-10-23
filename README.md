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
cada pull request; mientras alguno no siga ese formato, la rama principal falla y no publica nada.
Un commit mal escrito que ya está en la rama se corrige añadiendo su hash a `ignore.sha` en
`GitVersion.yml` junto con un commit nuevo del tipo correcto. GitVersion toma como actual el commit
de fecha más reciente, así que cada commit necesita una fecha de commit posterior a la de toda su
historia, algo que la CI también comprueba. Si la fecha se fija a mano, sirve
`GIT_COMMITTER_DATE=@$(( $(git log --format=%ct | sort -n | tail -1) + 1 ))`.

Solo los commits que suben la versión publican una nueva: cuando el último commit de la rama
principal pasa todas las pruebas y su versión es mayor que todas las publicadas, la CI crea
`v<versión>` en la página *Releases* del repositorio, con los archivos, un `SHA256SUMS` y las notas
agrupadas en cambios incompatibles, novedades y correcciones. Una versión publicada no cambia nunca
y volver a lanzar una ejecución antigua no publica nada. Las demás ramas y las pull requests llevan
una prerelease de la versión siguiente cuyo número crece con cada commit (`0.3.1-branch-rama.2`,
`0.4.0-feature-rama.1`, `0.3.1-PullRequest12.1`) y no se publican. `--version`, el daemon por
D-Bus, el servidor y los paquetes de Plasma y KWin muestran la versión; una compilación local sin
`WPRS_VERSION` usa la de `Cargo.toml` con el sufijo `-local`.

## Instalación (sin root)

Cada versión de la página *Releases* trae siempre los mismos nombres de archivo y un `SHA256SUMS`
para comprobarlos. Descarga `wallpaper-rs-x86_64-linux.tar.gz` de la última y ejecuta:

```sh
tar xzf wallpaper-rs-x86_64-linux.tar.gz
cd wallpaper-rs-*-x86_64-linux
./install.sh
```

Instala todo en tu directorio personal: binarios en `~/.local/bin`, servicio de usuario de
systemd, activación por D-Bus, la app de escritorio (con su lanzador e icono), el plugin de fondo
y el icono de la bandeja de Plasma. Funciona también en distribuciones con
sistema de archivos inmutable. Para desinstalar: `./uninstall.sh` (añade `--purge` para borrar
también la configuración).

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
*Ajustes…* la abre directamente en su página *Ajustes*, con el modo de ajuste, el color de fondo y
la lista de cada pantalla o de todas a la vez, los ajustes de cada escena, los fotogramas por
segundo, las pausas y las transiciones. Lo mismo hace `wprs-app --page settings` (o la acción
*Ajustes* del lanzador); `--page` acepta también `library`, `playlists`, `profiles`, `providers`,
`repos` y `account`. Si la app ya está abierta, pasa a esa página, salvo que el editor de imágenes
tenga cambios sin guardar: entonces solo se muestra la ventana.

Requisitos: KDE Plasma 6.4 o posterior, PipeWire, GStreamer (con los plugins base y good y el de
PipeWire, que lleva el sonido de los vídeos y de las escenas desde su versión 1.2; sin él o con uno
anterior, el sonido va a la salida por defecto; si aún no hay ninguna salida de sonido, el fondo
suena en cuanto aparece una) y un controlador Vulkan u OpenGL. El plugin de fondo usa los módulos QML de KPipeWire,
KWindowSystem y plasma-workspace; en Debian y Ubuntu vienen en `qml6-module-org-kde-pipewire`,
`qml6-module-org-kde-kwindowsystem` y `plasma-workspace`. La app de escritorio usa Qt 6 Quick,
Kirigami y el estilo de escritorio de KDE (`qml6-module-org-kde-kirigami` y
`qml6-module-org-kde-qqc2desktopstyle`).

## Instalación con Flatpak

La misma página *Releases* trae `wallpaper-rs.flatpak`, con el daemon, `wprs-ctl`, GStreamer y
los códecs dentro de un Flatpak, sin la app de escritorio. El runtime (`org.freedesktop.Platform`
26.08) se descarga de Flathub al instalarlo:

```sh
flatpak install --user ./wallpaper-rs.flatpak
flatpak run --command=wprs-plasma io.github.wallpaper_rs.WallpaperRs | sh
```

Flatpak no puede instalar el plugin de fondo ni el icono de la bandeja de Plasma: la última orden
los instala con `kpackagetool6` desde los archivos del propio Flatpak (sin `| sh` solo muestra las
órdenes). Hay que repetirla después de cada actualización y reiniciar Plasma. El plugin arranca el
daemon por D-Bus cuando lo necesita; no hay servicio de systemd. No instales a la vez el Flatpak y
el `.tar.gz`, porque los dos registran el mismo servicio D-Bus: ejecuta antes `./uninstall.sh`.

Sin la app, el icono de la bandeja ofrece solo lo básico (pausar, silenciar, el volumen, el perfil
y, en cada pantalla, *Siguiente*, *Elegir…* y *Favorito*) y, en lugar de *Abrir wallpaper-rs* y
*Ajustes…*, recuerda que lo demás (modo de ajuste, color de fondo, listas, ajustes de escena,
fotogramas por segundo, pausas y transiciones) se cambia con `wprs-ctl`:

```sh
alias wprs-ctl='flatpak run --command=wprs-ctl io.github.wallpaper_rs.WallpaperRs'
wprs-ctl --help
```

Permisos: la GPU, el socket de PipeWire, que publica los streams, lleva el sonido de los vídeos y de
las escenas y permite capturar el sonido solo para las escenas con permiso (no hace falta el de
PulseAudio), el socket de Wayland, solo para leer la resolución de cada pantalla (el daemon no abre
ventanas), el nombre D-Bus del daemon, el bloqueo de pantalla y la batería, el llavero del sistema (Secret
Service) para las contraseñas y claves de los proveedores, KWin para cargar el script del puntero
solo mientras una escena con permiso o un paralaje que sigue al puntero lo usa, lectura de
Imágenes, Vídeos y Descargas, y red. La red solo se usa para los proveedores de fondos y los
repositorios que añadas tú. Los cambios de zona horaria llegan sin permisos, por el fichero que
Flatpak actualiza en el sandbox. Para usar fondos de otra carpeta:
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
su IPC. El Flatpak no sirve en estos escritorios: no dejan usar `wlr-layer-shell` ni la
información de ventanas a las aplicaciones aisladas.

## Sesiones X11

Con `DISPLAY` y sin sesión Wayland ni Plasma, `wprsd` pone en cada monitor de RandR una ventana de
escritorio debajo de todas las demás (si el gestor de ventanas no las coloca, una ventana
*override-redirect* que se mantiene al fondo) y dibuja en ella con la GPU. Los monitores se llaman
como en `xrandr --listmonitors` y se siguen al conectarlos o quitarlos. Con imágenes fijas también
pone el fondo de la ventana raíz (`_XROOTPMAP_ID` y `ESETROOT_PMAP_ID`) para los compositores y las
terminales con transparencia, y los vídeos se pausan cuando una ventana del escritorio actual está a
pantalla completa o maximizada en ese monitor según EWMH. Se arranca con `wprsd &` en `~/.xinitrc`,
antes del gestor de ventanas, o desde el arranque automático de este; con el servicio de usuario,
`systemctl --user import-environment DISPLAY && systemctl --user restart wprsd`. Si el gestor de
ventanas arranca después que `wprsd` (o se cierra), las ventanas pasan solas a ser ventanas de
escritorio gestionadas (o *override-redirect*). Cuando una ventana deja de tapar el fondo, se dibuja
otra vez el último fotograma, también con el vídeo en pausa, sin reiniciarlo.

## Android

La misma página *Releases* trae `wallpaper-rs-android.apk`, un APK para Android 12 o posterior (64
bits). Siempre se firma con la misma clave, así que cada versión se instala encima de la anterior y
conserva el fondo elegido. Ábrelo en el teléfono (Android pedirá permitir la
instalación desde esa app) o instálalo desde un ordenador con
`adb install -r wallpaper-rs-android.apk`.

Para ponerlo de fondo, abre la app **wallpaper-rs**. La pestaña *Biblioteca* muestra las fotos y
los vídeos añadidos con *Añadir*, que abre el selector de fotos de Android (se pueden elegir varios a
la vez) y los copia al almacenamiento privado de la app, así que no hace falta ningún permiso y la app
no usa la red salvo para descargar el modelo de profundidad cuando lo pides y para tu servidor propio
si lo añades. Un mismo archivo no se
copia dos veces. Al tocar uno se abre su página, con una vista
previa en vivo, sus medidas y duración, y los botones *Usar como fondo*, favorito, *Añadir a una
lista* y *Quitar*. *Usar como fondo* abre la vista previa del sistema, donde se elige si va en la
pantalla de inicio, en la de bloqueo o en las dos; si la app ya es el fondo, el cambio entra con un
fundido sin pasar por ella. También se llega desde los fondos animados del sistema. Se dibuja con la
GPU (Vulkan, u OpenGL ES si Vulkan falla) con el mismo renderizador que el escritorio, y se recupera
sola si el sistema le quita la GPU o la superficie. Admite JPEG, PNG, WebP, GIF y BMP.

La pestaña *Listas* crea listas de fondos que cambian solos cada 15 o 30 minutos, cada hora, cada 6
horas, cada día o nunca, en orden o al azar y, si se quiere, saltando los vídeos con batería. El
cambio se decide cuando el fondo vuelve a verse, sin alarmas ni despertar el teléfono. La biblioteca,
las listas y sus reglas usan el mismo modelo y la misma lógica de rotación que el escritorio.

La pestaña *Ajustes* elige cómo se ajusta la imagen (rellenar, imagen completa, estirar o centrar),
el fundido entre fondos, el sonido de los vídeos, los fotogramas por segundo con batería (30, 15 o
10) y con el cargador (60 o 30), cuándo cambian los colores del sistema y los gráficos (automático,
Vulkan u OpenGL ES). Debajo, el diagnóstico muestra la GPU, los fotogramas dibujados y por segundo,
el límite que se aplica en ese momento, el estado de carga, ahorro de energía y temperatura y, con
una escena, la clase del teléfono y el nivel de calidad. *Licencias* muestra los textos de ONNX
Runtime, de los componentes que incluye y del modelo de profundidad.

Los vídeos (MP4, MOV, 3GP, WebM o MKV) los decodifica el hardware del teléfono directamente sobre
el fondo, sin pasar por la GPU ni copiar fotogramas, y se repiten en bucle. Se pausan en cuanto el
fondo deja de verse y sueltan el decodificador si siguen ocultos más de 30 segundos; al volver
siguen por donde iban. Si inicio y bloqueo muestran el mismo vídeo, comparten un solo reproductor.
Van sin sonido salvo con *Reproducir el sonido de los vídeos*. Salvo con el ajuste *Estirar*, piden
a Android que llenen la pantalla recortando los bordes; ese recorte lo hace el compositor del
sistema, así que un teléfono que no lo respete los mostrará estirados.

En la página de una imagen, *Moverse con el teléfono* activa el paralaje. Sigue la inclinación con
el sensor de rotación que no usa la brújula o, si no lo hay, con el normal; si el teléfono no tiene
ninguno o no da datos, sigue el paso de páginas del lanzador. La inclinación se
filtra y se recentra sola como en el escritorio, con el mismo muelle y el mismo sombreador: con un
mapa de profundidad lo cercano se mueve más que el fondo, y sin él la imagen entera se desplaza un
poco. El sensor solo se enciende mientras se ve un fondo con paralaje, a los fotogramas por segundo
permitidos (60 como mucho), y se apaga al taparlo, con el ahorro de energía, con poca batería o con
el teléfono muy caliente. Con el teléfono quieto no se dibuja ningún fotograma, y con batería el
sombreador da como mucho 8 pasos de profundidad.

*Calcular un mapa de profundidad* usa el mismo modelo que el escritorio, Depth Anything V2 Small. La
primera vez pide permiso para descargarlo y muestra su tamaño (99,1 MB), su licencia (Apache-2.0)
y el servidor (github.com). La descarga lleva el User-Agent genérico del escritorio, se reanuda si
se corta, se comprueba con su SHA-256 fijado y se guarda con el texto de su licencia al lado. El
mapa se calcula en segundo plano en el propio teléfono con ONNX Runtime, que va dentro del APK sin
su módulo de telemetría, y la foto no sale del teléfono. *Usar un archivo de mapa de profundidad*
importa un mapa hecho en el escritorio, el PNG de 16 bits que guarda junto a la imagen (por ejemplo
`foto.jpg.depth.png`), si tiene la misma proporción. En *Ajustes*, *Permitir descargar el modelo de
profundidad* (`models.allow_download`) bloquea la descarga, y ahí mismo se puede borrar el modelo.

El filtro *Escenas* de la biblioteca tiene *Añadir las escenas de ejemplo*, que instala las cinco
escenas que trae el APK (las mismas que el escritorio), y *Abrir un paquete de escena*, que importa
un `.wpak` de escena con el selector de documentos de Android. Se dibujan con el mismo motor que el
escritorio: capas, efectos, partículas (en la GPU si admite sombreadores de cálculo y, si no, en la
CPU con menos partículas), textos, líneas de tiempo y propiedades. La página de una escena muestra
sus propiedades y sus ajustes predefinidos, que se guardan en la misma configuración que el
escritorio. El nivel de calidad sale de la clase del teléfono (básica con poca memoria o sin
sombreadores de cálculo, alta si el fabricante declara una clase de rendimiento de Android 13 o
posterior, media en el resto) y de la batería: con batería baja un nivel, y con calor baja más. Una
escena demasiado pesada para el teléfono se queda quieta en su fotograma de muestra. Cada escena
dibuja a sus propios fotogramas por segundo, como mucho a los de los ajustes, solo mientras algo
cambia y nunca mientras no se ve. Los vídeos dentro de una escena los decodifica el hardware y se
copian a la GPU fotograma a fotograma, sin sonido, y se pausan si la escena se queda quieta. Los
scripts de las escenas no se ejecutan en el teléfono: la escena se ve sin lo que mueven. Sus
sonidos tampoco suenan en el teléfono, así que su volumen no aparece entre las propiedades.

Las miniaturas de los paquetes para adultos salen difuminadas y su página no los muestra hasta
pulsar *Mostrar igualmente*. Un paquete fotosensible no se previsualiza ni se puede usar como fondo
hasta aceptar el aviso de que destella o muestra patrones rápidos que pueden provocar crisis a
personas con epilepsia fotosensible; las listas lo saltan mientras tanto. Cada decisión se recuerda
por paquete. La biblioteca y la página de un paquete muestran su título y su descripción en el
idioma del teléfono, y la página avisa si sus scripts o sus sonidos no funcionan en él.

*Reaccionar al sonido* y *Reaccionar al tacto* solo aparecen en las escenas que los usan y se
activan escena a escena. El sonido se lee con el visualizador de Android, que da un espectro de
poca resolución de lo que suena en el teléfono; Android pide el permiso del micrófono la primera vez
que se activa y muestra su indicador mientras escucha. Solo escucha mientras se ve una escena con
ese permiso, y solo calcula bandas en memoria. Tras dos segundos de silencio deja de leerlo hasta que
algo empieza a sonar; si un reproductor sigue activo en silencio, lo mira cada dos segundos. El tacto
son los toques sobre el fondo en la pantalla de inicio, también solo en las escenas con ese permiso.

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
wprs-ctl pause | resume | toggle                # pausar los fondos animados
wprs-ctl volume 40 | mute | unmute              # sonido de los vídeos y las escenas
wprs-ctl config                                 # ver los ajustes
wprs-ctl config transition.duration_ms 1000     # cambiar un ajuste
wprs-ctl state                                  # estado en JSON
wprs-ctl reload                                 # releer la configuración
```

Modos de ajuste: `fill`, `fit`, `stretch`, `center`, `tile` y `span` (una imagen sobre todas las
pantallas, con su tamaño físico y los marcos entre ellas). Ajustes: transición (`fade` o
`none` y su duración), fotogramas por segundo y cuándo pausar (ventana encima, batería,
bloqueo). Un ajuste desconocido termina con error y muestra la lista de ajustes válidos.

`playback.max_fps` vale `display` por defecto (*Igual que la pantalla* en la app):
con corriente cada pantalla se dibuja como mucho a su frecuencia de refresco (la del modo de
`wl_output`, la de RandR en X11 o, en Plasma, la que da KWin) y con batería a 30 fotogramas por
segundo; un número fija el tope (`wprs-ctl config playback.max_fps 60`). Salvo en los vídeos, la
GPU pone su propio límite: el render por software se queda en 24 (15 con batería), una GPU
integrada en 60 (30 con batería) salvo escenas ligeras con corriente, y una dedicada sigue a la
pantalla hasta 240 (60 con batería). Los vídeos se quedan en 30 con `display` y render por
software y nunca pasan de sus propios fotogramas, nada se dibuja si nada cambia y `wprs-ctl state`
muestra la frecuencia (`refresh`) y el tope (`fps`) de cada pantalla.

Los streams llevan cada fotograma en memoria de la GPU (DMA-BUF) cuando el controlador Vulkan
puede exportarla y el consumidor (KPipeWire en Plasma) importarla: el daemon dibuja directamente en
los búferes del stream, sin leer el fotograma de vuelta ni copiarlo con la CPU, también con batería.
Si algo falta (OpenGL, render por software, un consumidor sin DMA-BUF), el stream usa memoria
compartida sin intervención; `WPRS_DMABUF=0` en el entorno del daemon la fuerza y `WPRS_DMABUF=1`
pide memoria de la GPU también con render por software. `wprs-ctl state` muestra el tipo de búfer
de cada pantalla (`buffers`: `dmabuf` o `memfd`).

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
wprs-ctl input grant org.example.dawn audio     # dejar que una escena siga el sonido
wprs-ctl input status                           # permisos y si se captura el sonido o el puntero
```

Las consultas aceptan `--json` e imprimen la respuesta del daemon tal cual; sin él muestran
tablas para leer, no para scripts. Lo que lanza un trabajo imprime su número y termina; con
`--wait` muestra el progreso y el resultado, y Ctrl-C deja de esperar sin parar el trabajo
(`wprs-ctl cancel <n>` lo para). Las contraseñas y claves de los proveedores se piden sin eco o
se leen de la primera línea de la entrada con `--secret-stdin`; nunca van en la orden. Códigos
de salida: 0 bien, 1 error, 2 uso incorrecto.

Las ediciones no tocan la imagen: se guardan en `<imagen>.wprs.json` a su lado (o en
`~/.local/share/wallpaper-rs/edits/` si la carpeta no se puede escribir), las aplica la GPU y el
daemon las vuelve a leer en cuanto cambian. La app las edita desde la ficha de cada imagen
(«Editar…») con vista previa en el fondo real, y cada acción del editor existe también en
`wprs-ctl edit`, que las cambia sin daemon:

```sh
wprs-ctl edit set foto.jpg ev=0.3 contrast=10 temperature=5200 tint=4
wprs-ctl edit crop foto.jpg 0.1,0.05,0.8,0.9 --rotate 90
wprs-ctl edit crop foto.jpg --aspect 16:9 -o DP-1      # recorte propio de una pantalla
wprs-ctl edit overlay foto.jpg '#ff8800' --blend soft-light --opacity 0.3 --mask linear:0.5,0,0.5,0.5
wprs-ctl edit preview foto.jpg ev=1                  # verlo en el escritorio 60 s sin guardar
wprs-ctl edit export foto.jpg -o foto-editada.png    # mismo resultado, sin daemon
wprs-ctl set foto.jpg -f span && wprs-ctl edit span foto.jpg --zoom 1.2
wprs-ctl config layout.bezel_mm 8                    # ancho del marco de cada pantalla al extender
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
archivos obtenidos por otra vía:

```sh
wprs-ctl models list                                 # tamaño, licencia, servidor y estado
wprs-ctl models download depth-anything-v2-small     # pregunta antes de descargar
wprs-ctl edit depth generate foto.jpg                # mapa calculado aquí; Ctrl-C cancela
wprs-ctl edit depth foto.jpg --dilate 4 --smooth 2   # ajustar cerca, lejos, curva y bordes
wprs-ctl edit parallax foto.jpg --source pointer --strength 0.02 --focus 0.3
```

`wprs-ctl pack` (crear, firmar, comprobar, actualizar y validar paquetes `.wpak`) y `wprs-ctl repo`
(claves, firma, publicación, rotación y verificación de un repositorio) funcionan sin daemon. La
contraseña de una clave se pide en la terminal o se lee de `WPRS_KEY_PASSWORD`.

La pestaña *Dispositivos* enlaza el teléfono con tu [servidor propio](#servidor-propio) con el
mismo cliente en Rust que el escritorio, para compartir la biblioteca, las listas y los ajustes con
tus ordenadores. Se escribe la dirección del servidor, *Enlazar este teléfono* muestra un código que
se aprueba en un ordenador ya enlazado (`wprs-ctl server approve` o la página *Cuenta* de la app) o
en la web del servidor, y después *Obtener la clave* trae la clave de sincronización de otro equipo
comparando seis cifras en las dos pantallas, como entre ordenadores. Desde el teléfono también se
aprueba un equipo nuevo, tras ver qué sistema y qué programa piden el código, y se le comparte la
clave. El token, la clave del teléfono y la de sincronización se guardan cifrados con una clave del
almacén de claves de Android que no sale del teléfono. Los registros van cifrados de extremo a
extremo y el servidor solo ve los equipos como `Device 1`, `Device 2`…

En el teléfono se aplican los títulos, etiquetas, valoraciones y favoritos de los fondos que tiene
(el mismo archivo en el teléfono y en el ordenador es el mismo fondo), las listas con sus reglas y
los ajustes compartidos, como el fundido. Se quedan en el teléfono el volumen, el sonido de los
vídeos, los fotogramas por segundo y el permiso para descargar modelos; los perfiles, los ajustes
predefinidos y los repositorios del escritorio se conservan sin tocar. Si cambias la clave en un
ordenador, el teléfono la recibe solo, y si se revoca un equipo, el primero que sincroniza (también
el teléfono) cambia la clave. La sincronización en segundo plano usa WorkManager cada hora, solo con
Wi-Fi u otra red sin límite de datos (salvo *Sincronizar también con datos móviles*) y sin batería
baja, más una tras abrir la app y otra 30 segundos después de cada cambio; *Sincronizar ahora* la
lanza al momento con cualquier red. Mientras el teléfono no está enlazado, la app no prepara ninguna
tarea ni toca la red al arrancar. *Pausar la sincronización* y *Quitar el servidor* (que pide al
servidor que olvide el teléfono) están en la misma pestaña.

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
pantalla, en la página *Ajustes* de la app o en la bandeja), y *Ocultar el contenido para adultos*
(`library.hide_mature`) los saca de las búsquedas y de los repositorios. Antes de aplicar un
paquete fotosensible, la app y `wprs-ctl` avisan y piden confirmación una sola vez por paquete, y
la app también lo permite desde sus detalles con *Permitir este fondo…*; hasta entonces el daemon
no lo aplica, un perfil no lo pone (la pantalla muestra el motivo) y las listas lo saltan. La
página *Ajustes* de la app y la bandeja avisan en la pantalla que lo muestra. La búsqueda filtra
por clasificación y por facetas que se calculan del propio paquete: las entradas que usa la escena
(`facet:audio`, `facet:pointer`, `facet:clicks`), el tamaño (`facet:4k`), `facet:photosensitive`,
`facet:android` si funciona entero en el teléfono, etc. Solo `media` y `network` salen de lo que
declara el autor en sus entradas, porque nada en el paquete muestra ese uso:

```sh
wprs-ctl lib show 1R1AB2                     # clasificación, origen, créditos, idiomas y descripción
wprs-ctl lib search 'content:general facet:audio'
wprs-ctl lib confirm photosensitive 1R1AB2   # permitir un paquete fotosensible
wprs-ctl lib confirm mature 1R1AB2           # dejar de difuminar su miniatura
wprs-ctl pack check fondo/                   # qué necesita, qué usa y qué declara un paquete
wprs-ctl pack upgrade fondo/ --rating general --photosensitive false   # pasar a la versión 2
```

`wprs-ctl pack build` calcula las funciones del motor que usa el paquete (`requires.features`) y las
escribe en el manifiesto; si un paquete pide una función o una versión que este wallpaper-rs no
conoce, dice que hace falta una versión más nueva (en el escritorio, con su miniatura). Las
versiones publicadas antes de la versión 2 del manifiesto no abren sus paquetes, pero siguen leyendo
los repositorios que los tienen: solo ven en ellos los de la primera versión. El validador comprueba
que las entradas declaradas incluyan las que la escena puede usar con cualquier valor de sus
propiedades y dice dónde las usa. Después de actualizar wallpaper-rs, la biblioteca vuelve a leer
una vez sus paquetes, así que los que pedían una versión más nueva empiezan a funcionar y los
anteriores reciben sus facetas. En el teléfono, la página de un paquete avisa si sus scripts o sus
sonidos no funcionan allí.

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
manifiesto. El servidor rechaza los paquetes para adultos, y su catálogo filtra por clasificación y
por faceta y avisa en la página de un paquete fotosensible.

El servidor construido así se identifica con la versión de `Cargo.toml` y el sufijo `-local`. Para
que muestre la de una versión publicada, se construye desde su etiqueta:

```sh
WPRS_VERSION=$(git describe --tags --exact-match --match 'v[0-9]*' | cut -c2-) docker compose up -d --build
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

La lista completa de variables y la API HTTP están en
[`docs/specs/server.md`](docs/specs/server.md).

### Enlazar los equipos

El servicio de cada equipo se enlaza con el servidor desde *Cuenta* en la app o con la CLI:

```sh
wprs-ctl server add https://fondos.example.org
wprs-ctl server link --name Escritorio          # muestra un código para /link o para:
wprs-ctl server approve BCDF-GHJK                # … en un equipo ya enlazado, tras ver quién lo pide
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
intenta entrar en tu cuenta. Las dos pantallas muestran el mismo número de seis cifras solo si
nadie se ha colado entre los equipos, y en cada uno escribes las cifras del otro: la clave no sale
ni se guarda hasta que coinciden en los dos; cada intento da una única oportunidad a quien lo
intente. Al revocar un equipo, otro cambia la clave de sincronización y vuelve a cifrar todo, así
que el equipo revocado ya no puede leer lo nuevo; la clave nueva pasa solo entre equipos que
compararon sus cifras, y los demás vuelven a unirse. Cambiar la contraseña en la web cierra las
demás sesiones y borra los tokens; los equipos siguen enlazados salvo que elijas cerrarlos también.

Se sincronizan los títulos, etiquetas, valoraciones y favoritos de la biblioteca, las listas, los
perfiles, los ajustes predefinidos y los ajustes (salvo el volumen, la caché, el paralaje por
software y el permiso para descargar modelos, que se quedan en cada equipo), cifrados de
extremo a extremo; sin cifrar solo con `--plaintext`, que avisa antes. El token y la clave de
sincronización viven solo en el llavero del sistema; el servidor ve los equipos como `Device 1`,
`Device 2`… y el nombre real viaja cifrado. Al enlazar se añade también el repositorio del
servidor.

Los repositorios que añade un equipo llegan a los demás como propuesta: cada equipo los añade solo
cuando lo confirmas, con su huella. Las claves de los proveedores viajan solo si lo eliges para
cada proveedor, solo en cuentas cifradas, y en cada equipo se guardan solo en su llavero. Tus
propios fondos (no los de un repositorio) suben cifrados por partes si eliges el archivo o su
carpeta; si se corta, la subida sigue donde se quedó, el mismo archivo se guarda una vez y los
demás equipos lo descargan cuando lo pides. Si dos equipos cambian una lista de formas que no se
pueden juntar, se conservan las dos: la otra queda como «Lista (conflicto)». Lo que una versión
más antigua no entiende se le devuelve tal cual.

Si temes perder todos los equipos, `server escrow` guarda la clave en el servidor cerrada con una
frase (Argon2id, 256 MiB). Quien consiga una copia del servidor puede probar frases sin límite: usa
varias palabras sin relación que no uses en ningún otro sitio. Tras revocar un equipo la frase deja
de valer: guarda otra nueva, porque quien conocía la anterior podría abrir lo que se cierre con ella.

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
wprs-ctl prop preset calma -o DP-1 && wprs-ctl prop reset -o DP-1
wprs-ctl scene bench escena.wpak --size 1920x1080 --frames 300   # coste por fotograma, sin daemon
```

En la página *Ajustes* de la app, el botón *Ajustes de la escena* de cada pantalla que muestra una
escena cambia sus propiedades al momento, aplica sus preajustes y da o retira el permiso de sonido
y de puntero. La carpeta `scenes/` guarda cinco escenas de ejemplo de dominio público (CC0):
capas, modos de fusión, máscaras, línea de tiempo y profundidad que sigue al puntero o una órbita
lenta con un mapa calculado a partir de sus formas (`dusk`), partículas, efectos y el sonido del
viento (`snowfall`), análisis del sonido (`pulse`), puntero y el sonido del agua (`pond`) y un
script (`orbits`). `scripts/ambient-sounds.sh` sintetiza esos dos sonidos (ruido filtrado y gotas,
también CC0) y los guarda como Ogg Vorbis.

Las escenas pueden tener sonido propio: capas `sound` con sonidos Ogg Vorbis, Opus, FLAC o WAV del
paquete (de 60 s como mucho cada uno), en bucle o de una vez, que empiezan y paran con una línea de
tiempo o una propiedad, con volumen, panorama y fundidos, mezcladas en un solo stream. Empiezan en
silencio, como los vídeos: suenan con `wprs-ctl unmute` o el botón de sonido de la app, de la
bandeja o de cada pantalla que muestra una escena con sonido, y siguen el volumen general y la
propiedad *Volumen* que aparece en los ajustes de cada escena con sonido. Solo suenan mientras la
escena se ve en una pantalla activa y sin tapar (se pausan y siguen con ella al taparla, aunque la
escena no se pause, al pausar o con batería, y se paran al quitarla), nunca con la pantalla
bloqueada, y una escena que se ve en varias pantallas suena en una sola. `install.sh` las instala en
`~/.local/share/wallpaper-rs/examples` (`--no-examples` las omite), el RPM en el paquete opcional
`wallpaper-rs-examples` y el Flatpak las incluye. Aparecen en la biblioteca y, si se quitan de
ella, no vuelven.

## Desarrollo

Dependencias de compilación (Debian/Ubuntu): `libpipewire-0.3-dev libclang-dev pkg-config
libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev qt6-base-dev qt6-declarative-dev`. Para las
pruebas: `mesa-vulkan-drivers pipewire wireplumber dbus-x11 gstreamer1.0-plugins-base
gstreamer1.0-plugins-good gstreamer1.0-pipewire gstreamer1.0-tools`, `sway grim foot` para la de
Sway, `xvfb x11-utils x11-xserver-utils xdotool imagemagick openbox` para la de X11 y
`gnome-keyring` para la del servidor.

```sh
cargo test --workspace                   # pruebas unitarias, de render y de vídeo
cargo build -p wprs-pipewire --example probe
cargo build -p wprsd -p wprs-ctl
cp target/debug/examples/probe target/debug/
scripts/integration-test.sh target/debug # D-Bus + PipeWire + daemon de punta a punta
scripts/sway-test.sh target/debug        # superficies layer-shell en un Sway sin pantalla
scripts/x11-test.sh target/debug         # ventanas de escritorio en un Xvfb
cargo build -p wprs-server && cargo build -p wprs-android --example phone
cp target/debug/examples/phone target/debug/
scripts/sync-test.sh target/debug        # daemons, el cliente del teléfono y wprs-server: enlace, clave y sincronización
scripts/package.sh                       # genera dist/wallpaper-rs-*.tar.gz
scripts/translations.sh update [idioma]  # actualiza po/ con los textos del QML
target/debug/wprs-app --check            # carga todo el QML de la app y falla ante cualquier aviso
```

APK de Android (con el SDK, el NDK 28 o posterior y `cargo install cargo-ndk`):

```sh
rustup target add aarch64-linux-android x86_64-linux-android
ANDROID_NDK_HOME=/ruta/al/ndk android/gradlew -p android assembleDebug
adb install -r -d android/app/build/outputs/apk/debug/app-debug.apk
android/gradlew -p android connectedDebugAndroidTest -Pabis=x86_64  # pruebas en un emulador x86_64
scripts/android-power.sh -o consumo.csv  # consumo, fps y memoria por escenario en el teléfono
```

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
| `crates/wprs-scene` | Modelo de las escenas: capas, valores enlazados, líneas de tiempo y efectos, sin GPU |
| `crates/wprs-pipewire` | Exportación de streams de vídeo PipeWire |
| `crates/wprs-video` | Reproducción de vídeo en bucle y del sonido de las escenas con GStreamer |
| `crates/wprs-wayland` | Superficies layer-shell y ventanas de escritorio X11, escala, ritmo y oclusión |
| `crates/wprs-audio` | Captura del sonido con PipeWire y su análisis en bandas para las escenas, también a partir del espectro que da Android |
| `crates/wprs-script` | Scripts de escena en JavaScript, aislados y con límites de CPU y memoria |
| `crates/wprs-sync` | Cifrado, relojes, fusión, protocolo, buzón y formato de los registros de la sincronización, compartidos por el escritorio y Android |
| `crates/wprs-android` | Fondo animado de Android en Rust: JNI, GPU compartida, una superficie por fondo, biblioteca, listas, ajustes, escenas, paralaje, mapas de profundidad y sincronización con el servidor propio |
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
| `deploy` | Despliegue de referencia del servidor con Caddy |
| `docs` | Especificaciones de la interfaz D-Bus, los paquetes y sus escenas, los repositorios firmados y el servidor |

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
- El modelo de profundidad y ONNX Runtime solo se descargan si lo pides y aceptas su tamaño,
  licencia y servidores; llevan el mismo User-Agent genérico, sin cabecera Referer ni redirecciones
  de HTTPS a HTTP, se verifican con SHA-256 y los mapas se calculan en el equipo, en un proceso
  aparte. ONNX Runtime funciona con su telemetría desactivada: no guarda ningún identificador ni se
  conecta a nadie. En Android, ONNX Runtime va dentro del APK sin el componente que arrancaba su
  telemetría, y la app solo usa la red para descargar el modelo después de que lo aceptes.
  `models.allow_download` se queda en cada equipo y, si la configuración no se puede leer, cuenta
  como desactivado.
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
- Ninguna escena oye el sonido ni sigue el puntero sin tu permiso, que se da y se retira por escena
  desde la app o `wprs-ctl input grant|revoke <escena> audio|pointer`. Cuando se aplica
  una escena que los pide sin permiso, desde donde sea (la app, la bandeja, una lista o `wprs-ctl`),
  la app y la bandeja preguntan una vez por escena qué permitir, explicando qué se capta y que nada
  se guarda ni se envía, y sus ajustes muestran un aviso con el botón *Permitir* hasta que decides;
  *Ahora no* se recuerda y no vuelve a preguntar. `wprs-ctl lib apply`, `playlist start`, `next` y
  `prev` imprimen la orden exacta para darlo. Aun con permiso, la captura del sonido (un stream de
  PipeWire llamado «wallpaper-rs audio analysis», que puede encender el indicador de micrófono) y el
  seguimiento del puntero solo funcionan mientras se ve una escena que los usa, y se paran al pausar
  o bloquear la pantalla. Al taparla se para el puntero, y también el sonido si está activa la pausa
  al tapar, como viene por defecto. Del sonido solo se calculan bandas y niveles en memoria: no se
  guarda, no se registra y no sale del equipo. Del puntero solo se guarda en memoria la última
  posición, y nunca se leen las teclas. Una imagen cuyo paralaje sigue al puntero lo usa con las
  mismas reglas, porque elegir esa fuente es el permiso. En Android, el permiso del micrófono se pide
  la primera vez que se activa *Reaccionar al sonido* en una escena, y solo se escucha mientras se ve
  una escena que lo usa y tiene ese permiso.
- Los scripts de las escenas se ejecutan en un proceso aparte, sin archivos, red ni otros
  procesos, con límites de memoria y de CPU por fotograma.
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
