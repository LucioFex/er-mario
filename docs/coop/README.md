# ER Mario en cooperativo: Mario + Luigi con Seamless Co-op y Elden Ring Reforged

Guía para dejar dos PCs listas para jugar juntos: el anfitrión juega con Mario y el invitado con Luigi.
Está escrita para seguirla paso a paso, también desde una sesión de Claude Code en cada PC.
Antes de pasar al paso siguiente, comprobá cada verificación (✔).

## Cómo funciona

- Cada jugador corre su propio Mario. El mod manda por Steam la pose de su personaje, unas 30 veces
  por segundo, por un canal propio (`SteamNetworkingMessages`, canal `0x4D41`). Así cada uno ve al
  otro como Mario o como Luigi.
- Mario usa la pechera *Vagabond Knight Armor* (660100) y Luigi la *Vagabond Knight Armor (Altered)*
  (661100). Cada PC arma también el modelo del otro personaje (modelo 996).
- Los enemigos son del anfitrión. Los golpes del invitado viajan al anfitrión, que les descuenta la
  vida y los remata.
  - El invitado no agarra enemigos (ni Bob-omb ni cola de Bowser) y tampoco los lanza por el aire.
  - Sus golpes igual pegan, y la barra de vida se ve bajar al instante.
- Un golpe al compañero lo empuja un poco. No le hace daño.
- **Revivir**: si te quedás sin vida y el compañero puede ayudarte, no morís: quedás tirado con
  estrellitas sobre la cabeza durante 30 s (arriba dice HELP y los segundos). El otro se acerca y
  mantiene **flecha abajo** un segundo (aparece ▼ RESCUE y un anillo que se llena): volvés con 3
  cuñas, el sonido del 1-UP y un giro en el aire. Si nadie llega, si caen los dos o si es una caída
  al vacío, es la muerte normal de Seamless (espectador y respawn).
- **Correa**: a más de 30 m aparece TOO FAR; desde 35 m cuesta alejarse y a 40 m no se puede
  (volver siempre se puede). Si quedan a más de 48 m durante 2 s (caída, Torrent, gancho), el que
  no está peleando (si están iguales, el invitado) aparece al lado del otro con humo y el sonido de
  la tubería. Nunca en peleas de jefe, caído, a caballo, en menús, puertas o escenas.
- La cámara de Lakitu se abre un poco y mira hacia el compañero cuando está cerca.
- Sin una sesión de Seamless, el mod funciona igual que en un jugador.

## Requisitos (en las dos PCs)

1. **Elden Ring en Steam**, actualizado. Los dos tienen que ser amigos en Steam.
2. **Elden Ring Reforged (ERR)**, la **misma versión** en las dos PCs (Nexus Mods, mod 541; se usó la
   **2.3.6.3**). Se descomprime fuera de la carpeta del juego. En esta guía `<ERR>` es la carpeta que
   tiene los `.BAT` de lanzamiento, por ejemplo `C:\ERR - Elden Ring Reforged\ERRv2.3.6.3`.
3. **Seamless Co-op**, la **misma versión** en las dos PCs (Nexus Mods, mod 510; se usó la 2.0.1).
4. **ER Mario, el mismo build** en las dos PCs, porque los paquetes llevan una versión. Para
   comprobarlo, comparar el SHA256 de `er_mario.dll` en las dos PCs: el chequeo de actualizaciones
   del mod sigue diciendo 0.4.2 aunque el build cambie. Bajar
   `ER-Mario-coop.zip` del pre-release
   [coop-2026-10-17](https://github.com/LucioFex/er-mario/releases/tag/coop-2026-10-17). Trae
   `er_mario.dll`, un `er_mario.ini` de ejemplo y las licencias. Si después sale otro build, los dos
   tienen que actualizar.
5. **Una ROM de Super Mario 64 (US) propia** en cada PC (`.z64`, `.n64` o `.v64`). El MD5 del
   `.z64` correcto es `20b854b239203baf6c961b850a4a51a2`.
   **La ROM no se comparte ni se sube a ningún lado.**

## Pasos en cada PC

### 1. Reforged

1. Descomprimir ERR fuera de la carpeta del juego. Abrirlo una vez con
   `2 - Launch ELDEN RING Reforged - Offline or Seamless (Windows).BAT` para comprobar que arranca, y
   cerrar el juego.
   - ✔ Existen `<ERR>\dll\offline` (con un `ersc_settings.ini` de Reforged) y `<ERR>\addons`.
   - Reforged arma su propio perfil de me3 en cada arranque, con los DLL de `dll\offline` y las
     carpetas de `addons`. No hay que editar ningún `.me3`.
   - El launcher necesita una ventana de consola propia: abrirlo siempre con el `.BAT`, nunca
     llamando a `ReforgedLauncher.exe` desde otra terminal (falla con
     `PromptPlus requires a terminal/console environment`).

### 2. Seamless Co-op dentro de Reforged

1. Del zip de Seamless, copiar **solo** `SeamlessCoop\ersc.dll` y la carpeta `SeamlessCoop\locale`
   a `<ERR>\dll\offline\`. Se usa el `ersc_settings.ini` que trae Reforged, no el del zip.
2. En `<ERR>\dll\offline\ersc_settings.ini` (igual en las dos PCs):
   - `cooppassword = <la que elijan>`: la **misma contraseña** en las dos.
   - `enemy_damage_scaling = 15` y `boss_damage_scaling = 15`: con dos jugadores los enemigos pegan
     un 15% más.
   - El resto se deja como lo trae Reforged: `save_file_extension = err`, `allow_invaders = 0`,
     `death_debuffs = 0` y el escalado de vida en 0.

**Dificultad:** solo cuenta con dos jugadores. De a uno el juego queda igual que siempre. De a dos:
- Los golpes de Mario quitan un porcentaje de la vida máxima del enemigo, así que con dos Marios
  todo moriría el doble de rápido. ER Mario hace que los enemigos aguanten como si tuvieran un 35%
  más de vida, y los jefes un 100% más. Esto se puede cambiar en `er_mario.ini` con
  `coop_health_scaling` y `coop_boss_health_scaling`.
- Los enemigos pegan un 15% más (`enemy_damage_scaling` de Seamless).

### 3. ER Mario dentro de Reforged

1. Copiar a `<ERR>\dll\offline\` el `er_mario.dll` y el `er_mario.ini` del zip.
2. En `er_mario.ini`:
   - La ROM: copiarla a `<ERR>\dll\offline\`, o apuntar a ella con `rom = C:\ruta\a\la\rom.z64`.
   - **PC de Luigi:** `character = luigi`
   - **PC de Mario:** sin esa línea, o `character = mario`
   - `base_assets = <ERR>\mod`, con la ruta completa. Reforged trae su propia versión de los íconos del
     menú (`menu\hi\01_common.tpf.dcx`) y de la lista de materiales
     (`material\allmaterial.matbinbnd.dcx`). Con esto, ER Mario parte de las de Reforged en vez de las
     del juego y no le pisa los cambios.
   - **Controles:** los dos con la configuración de botones de Elden Ring por defecto (Sistema >
     Controles > Configuración de botones > restablecer). La flecha abajo del mando aprieta el botón
     de interactuar del juego, que por defecto es Y. Si alguien lo cambió, poner `pad_interact = A`
     (o B, Y, LB, RB, BACK, START) en su `er_mario.ini`.
3. Abrir con el `.BAT` *Offline or Seamless* hasta el menú principal y cerrar. **La primera vez hacen
   falta dos arranques:** en el primero el mod arma los modelos a partir de la ROM; en el segundo
   me3 ya los carga.
   - ✔ En `<ERR>\dll\offline\logs\er_mario.log` aparece `package folder <ERR>\addons\er-mario`.
   - ✔ También aparece `assets: /menu/hi/01_common.tpf.dcx from ...\mod`.
   - ✔ Existe `<ERR>\addons\er-mario\parts\bd_m_0996.partsbnd.dcx`, que es el modelo del compañero.
   - ✔ No aparece `LOOKS USED`. En Reforged 2.3.2.2 y 2.3.6.3 se comprobó que las filas que reutiliza
     el mod (bullets 65-70 y AtkParam 994-999) están libres. Las pecheras 660100 y 661100 conservan
     las estadísticas de Reforged; el mod solo les cambia el modelo.
   - Si el log dice `assets: build failed: unsupported DCX format`, el `er_mario.dll` es viejo:
     Reforged comprime su lista de materiales con DEFLATE y los builds anteriores al 18/10 no la leían.
     Bajá el zip de nuevo, reemplazá `er_mario.dll` y arrancá dos veces más. El build que falló no
     deja nada a medias: se rehace solo.
   - ✔ Ya en el mundo aparece `combat: enemies count as bosses from 5000 HP (x2, Reforged)`. Reforged
     les da a los enemigos ~1,7 veces la vida de vanilla; sin esto, los soldados comunes contaban como
     jefes (el uppercut no los levantaba y los golpes les sacaban muy poco). Si hace falta, se ajusta
     con `enemy_hp_scale = 2` en `er_mario.ini`.

### 4. Una sola partida para solo y coop (PC de Mario, el anfitrión)

Mario juega solo en Reforged y de vez en cuando comparte esa misma partida con Luigi. Reforged usa
`ER0000.err` tanto en solo como con Seamless (`save_file_extension = err`), así que **es el mismo
archivo** y no hay que copiar nada entre una cosa y la otra. Para jugar solo, simplemente no se usa
la olla.

La primera vez hay que pasar a ese archivo la partida actual de ER Mario:

1. Con el juego cerrado, ir a `%APPDATA%\EldenRing\<número de Steam>\`.
2. **Copia de seguridad:** copiar toda la carpeta a otro lado antes de tocar nada. Ojo: el launcher
   de Reforged puede haber creado ya un `ER0000.err` a partir de la partida vanilla (`ER0000.sl2`).
3. Reemplazar `ER0000.err` por una **copia** de `ER0000_mario.sl2` (la partida de ER Mario en un
   jugador), renombrada a `ER0000.err`. El original `ER0000_mario.sl2` queda como respaldo de la
   partida vanilla.
   - ✔ Al arrancar Reforged, *Continuar* carga el personaje de Mario donde estaba.
4. Reforged cambia armas, objetos y enemigos. Una partida vanilla carga, pero conviene revisar el
   equipo. El launcher de Reforged guarda hasta 25 copias de la partida por su cuenta.
5. Desde ahí, para jugar solo con Mario se usa el `.BAT` *Offline or Seamless*. El perfil
   `er-mario.me3` de la instalación vanilla de ER Mario queda para la partida vanilla.
   - ✔ Jugando solo, en el log aparece `coop: session []`.

La PC de Luigi puede crear un personaje nuevo en su propia partida de Reforged. El progreso del
mundo es del anfitrión.

### 5. Jugar

1. Los dos abren Reforged con el `.BAT` *Offline or Seamless* y cargan su personaje.
2. Mario (anfitrión) usa la **Tiny Great Pot** (Gran olla pequeña). Luigi usa el **Effigy of Malenia**
   (Efigie de Malenia) para unirse. Las dos cosas las da Seamless. Con Mario se usan desde el menú:
   **Inventario > el objeto > Usar** (la X del mando es el puñetazo de Mario y no usa objetos). En el
   log aparece `input: the game asks to use an item (let through ...)`.
   La cámara de Lakitu gira como la de Elden Ring: si a alguien le resulta al revés, que cambie en
   **Sistema > Cámara** la opción de invertir el eje X (es de cada PC).
3. ✔ En el log de cada uno aparece:
   - `net: Steam messages API found, channel 0x4d41`
   - `coop: session [...] , host ... (us)` en la PC de Mario y `(them)` en la de Luigi
   - `coop: pose from ...` cuando llega la pose del otro

## Prueba rápida del canal de Steam (una sola PC)

Antes de jugar se puede comprobar en cada PC que el canal de Steam funciona. Poné
`coop_test = loopback` en `er_mario.ini` y arrancá hasta el menú principal. La PC se manda paquetes
a sí misma.
- ✔ En el log aparecen `coop test: loopback got hello`, `got pose`, `got hit`, `got shove` y
  `loopback to ...: N sent, M received`.
- Después **borrá esa línea**. `coop_test = guest` hace jugar como invitado (sin agarres y con los
  enemigos clavados en 1 de vida); solo sirve para pruebas.

## Qué mirar en el log si algo falla

| Línea del log | Qué significa |
|---|---|
| `net: steam_api64 lacks the messages API` | El juego no tiene la API de red: no se puede ver al otro como Mario |
| `coop: session [] ...` con los dos conectados | La sesión de Seamless no está en `CSSessionManager`: falta investigarlo |
| sin `coop: pose from` | No llegan los paquetes: revisar que los dos tengan el mismo `er_mario.dll` y que sean amigos en Steam |
| `coop: packets of version N` | Tienen versiones distintas de ER Mario (hace falta el mismo build en las dos PCs; ahora es la 6) |
| `coop: status from ... partner card on` | Llega el estado del otro (vida, monedas, poder, estilo): la tarjeta del compañero funciona |
| `coop: pose from ... with its place` | El otro se ve suave y en su lugar real. Si dice `no place`, se ve donde el juego tiene a su personaje (más tironeado) |
| `coop: partner ... shown at their own place (...)` / `where the game has them` | Dónde se dibuja al compañero y a cuántos metros está la copia del juego. Si al cargar se ven encimados, esta línea dice por qué |
| `net: last 20 s sent pose ... fx ...; got pose 29.x/s ...; lost poses N, effects M` | Tráfico de red: lo normal es pose ~30/s y pocas perdidas. Muchas `lost` = paquetes que no llegan |
| `revive: down ...` / `revive: revived by ...` / `revive: reviving ...` | Quedaste caído / te revivieron / revivís al otro |
| `revive: nobody came in time` / `nobody left to revive` | Se acabó el tiempo o cayeron los dos: muerte normal |
| `leash: N m apart for 2 s: Mario warps to the partner` | La correa te llevó al lado del compañero |
| `input: interact (D-pad down) pressed, no event animation ... possible actions ...` | La flecha abajo no hizo nada: dice qué acciones permitía el juego, tipo de personaje, si está en el piso y a qué distancia de Mario está el Tarnished |
| `follow: interact started anim N ... on D-pad down (allowed ...)` | La flecha abajo sí abrió una puerta/agarró algo |
| `coop: BROS! (both on one enemy)` | Los dos le pegaron al mismo enemigo dentro de 1,5 s |
| `coop: partner ... is down` / `is back` | El compañero cayó / volvió |
| `coop: partner's character paired by elimination` | Normal si hay un solo compañero: el personaje no traía el SteamID donde se esperaba |
| `combat: a partner's ... not here` | Un golpe del invitado sobre un enemigo que el anfitrión no tiene cargado (se ignora) |
| `combat: attack 99X as found: ... LOOKS USED` | Reforged usa esa fila de ataque: avisar, porque hay que mover las filas del mod |
| `equip: armour rows as found` | Modelos originales de las pecheras 660100 y 661100 en Reforged |
| `juggle: cXXXX not lifted: ...` | Por qué el uppercut (X, X, Y) no levantó a ese enemigo (jefe, muy grande, etc.) |
| `combat: left out cXXXX, invisible (...)` | Un personaje invisible que Mario ignora (ayudantes de las gracias, enemigos que el juego no dibuja) |
| `perf: slow ... \| havok: ...` | Un tirón del mod; la parte final dice cuántos cuerpos de colisión miró |

## Perfil de me3 sin Reforged

Para jugar en cooperativo con el juego vanilla (sin Reforged), está `docs/coop/er-mario-coop.me3`.
Carga `SeamlessCoop/ersc.dll` y `er_mario.dll`. Los dos jugadores tienen que usar lo mismo:
Reforged los dos, o vanilla los dos.
