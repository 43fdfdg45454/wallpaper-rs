# Guía para hacer fondos web

Un fondo web es una página que wallpaper-rs muestra detrás de las ventanas. Todo lo que usa va
dentro del paquete: no hay servidor, la red está cortada salvo que la declares y el usuario la
permita, y la página no puede abrir ventanas, diálogos ni descargas. `docs/specs/package.md`
("Web pages") describe el manifiesto y `docs/specs/web.md` el motor y el aislamiento; el ejemplo
`scenes/lights` es un fondo web completo.

## Lo mínimo

```
mi-fondo/
  manifest.json
  index.html
  app.js
  preview.png
```

```json
{
  "schema": "wallpaper-rs.wpak/2",
  "id": "org.example.reloj",
  "version": "1.0.0",
  "title": { "en": "Clock", "es": "Reloj" },
  "default_locale": "en",
  "license": "CC0-1.0",
  "origin": { "kind": "original" },
  "content": { "rating": "general" },
  "photosensitive": false,
  "kind": "web",
  "entry": "index.html",
  "previews": { "thumbnail": "preview.png" },
  "inputs": [],
  "web": { "background": "#000000" },
  "properties": {
    "color": { "type": "color", "default": "#ffffff", "label": { "en": "Color", "es": "Color" } }
  }
}
```

`wprs-ctl pack check mi-fondo` explica lo que falta, `wprs-ctl pack build mi-fondo -o reloj.wpak`
calcula `requires.features` y arma el paquete, y `wprs-ctl lib import reloj.wpak` lo deja en la
biblioteca. Las rutas son relativas: `<script src="app.js">`, `fetch("datos.json")`.

## La interfaz `window.wprs`

Está lista antes de tus scripts:

```js
wprs.onProperties((cambiadas) => {
  if (cambiadas.color) document.body.style.color = cambiadas.color.hex;
});
wprs.onPause((pausada) => pausada ? detener() : seguir());
wprs.onSettings(({ fps, locale, volume }) => ajustar(fps, locale, volume));
```

- `onProperties` recibe todas las propiedades al registrarse y después solo las que cambian. Un
  color llega como `{ hex, r, g, b, a }` (de 0 a 1) y un combo como `{ value, label }`.
- `requestAnimationFrame` ya va al ritmo que elige el usuario (y más lento con batería): no hace
  falta limitarlo. Una página que no cambia no gasta nada, así que dibuja solo cuando haga falta.
- Con la página oculta, `document.visibilityState` es `"hidden"`, `onPause(true)` avisa y los
  fotogramas se detienen; tras 5 minutos oculta se cierra y vuelve a cargarse al mostrarse.
- `onAudio(({ left, right }) => …)`: 64 bandas por canal, de graves a agudos, unas 30 veces por
  segundo, cuando declaras `"inputs": ["audio"]` y el usuario lo permite.
- El puntero llega como eventos normales (`pointermove`) si declaras `pointer`, y los clics
  (`click`) si declaras `clicks`, solo desde clics reales y en las pantallas donde el usuario los
  activa. El teclado nunca llega.
- `localStorage` e IndexedDB guardan entre arranques si declaras `"web": { "storage_kib": 64 }`
  (hasta 5120) y el usuario no lo oculta; sin eso, nada sobrevive.
- `console.log` y los errores van al registro del servicio (`RUST_LOG=wprsd=debug`).
- `wprs.volume` es el volumen del fondo para el sonido de tu página: puede sonar sin que nadie
  toque nada, `<audio>`, `<video>` y `AudioContext` suenan a ese volumen (por el que les pongas) y
  a 0 está silenciada. Los audios y vídeos del paquete o de la carpeta elegida se reproducen desde
  una copia en memoria, de hasta 256 MiB cada uno.

## Carpetas elegidas

Una propiedad `folder` deja que el usuario elija una carpeta de fotos o vídeos. La página nunca ve
rutas, sino direcciones `wpak-user://` que solo sirven dentro de ella:

```js
const una = await wprs.randomFile("fotos");
imagen.src = una;
```

Con `"mode": "all"` también tienes la lista entera y los cambios:

```js
const todas = await wprs.files("fotos");
wprs.onFiles("fotos", ({ added, removed, changed }) => actualizar(added, removed, changed));
```

`changed` trae pares `[antes, ahora]` de los archivos que se reescribieron con el mismo nombre: el
token nuevo carga el contenido nuevo.

## Lo que ve tu página

Por defecto ve la zona horaria (`new Date()` da la hora local), el idioma (`navigator.language`),
lo que puede hacer la tarjeta gráfica a través de WebGL (en el teléfono, también su modelo) y su
propio almacenamiento. El usuario puede
ocultar cada cosa: entonces la página ve UTC, `en-US`, un nombre de tarjeta genérico (WebGL sigue
funcionando) y nada guardado. Haz que funcione en todos esos casos.

## Red

Si de verdad la necesitas, declara los orígenes (hasta 8, `https://` sin ruta) y `network` en
`inputs`:

```json
"inputs": ["network"],
"permissions": { "network": { "allow": ["https://api.example.org"] } }
```

La página solo llega a esos orígenes y solo después de que el usuario lo permita; mientras tanto
`fetch` falla. Evítala si puedes: un fondo sin red funciona en todas partes y no expone nada.

## En el teléfono

La app de Android muestra el mismo paquete con el WebView del sistema (el motor de Chrome que
Android mantiene al día), con la misma interfaz `window.wprs`. Conviene tenerlo en cuenta:

- La página vive en una dirección `https://…wpak.invalid/` y los archivos elegidos llegan como
  `https://…/:user/<propiedad>/…` en lugar de `wpak-user://`: trátalos como direcciones opacas y usa
  rutas relativas para tus archivos.
- `requestAnimationFrame` va al ritmo que elige el usuario, pero las animaciones CSS y los vídeos van
  al de la pantalla: para ahorrar batería anima con `requestAnimationFrame`.
- El dedo es el puntero (`pointermove`, como un ratón que pasa por encima) si el usuario activa
  *Reaccionar al tacto*, y un toque sin moverse es un `click` si activa *Reaccionar a los toques*.
  Los toques llegan después del primer dibujo de la página.
- El sonido de la página suena solo si el usuario activa el sonido de los vídeos; si no,
  `wprs.volume` es 0.
- `RTCPeerConnection` no existe y el `console` va a logcat (`adb logcat -s wprs`).
- WebGL depende del teléfono: comprueba que `getContext("webgl")` no devuelve `null` y ten un dibujo
  más sencillo para ese caso.

## Pasar una aplicación a fondo web

wallpaper-rs nunca ejecuta programas de un paquete. Un fondo hecho como aplicación (un ejecutable
que dibuja detrás de los iconos) se pasa así:

1. Exporta el proyecto a web desde su motor: los motores de juegos y las bibliotecas 3D más usadas
   generan HTML con WebGL o WebAssembly (`.wasm`, que debe empezar por su firma `\0asm`).
2. Pon el resultado en la carpeta del paquete con `index.html` como `entry` y cambia las rutas
   absolutas por relativas.
3. Sustituye lo que leía del sistema por la interfaz: sus opciones por `properties` y
   `onProperties`, su análisis de audio por `onAudio`, su ratón por los eventos del puntero.
4. Quita lo que no se puede: archivos del equipo (usa una propiedad `folder` o `file`), red sin
   declarar, ventanas, teclado y procesos.
5. Comprueba que dibuja poco: limita el trabajo por fotograma, pausa en `onPause` y no hagas
   bucles con `setInterval` rápidos, que se notan en la batería.

Los ejecutables y las bibliotecas (exe, dll, so, sh, bat, AppImage…) se rechazan al armar el
paquete.
