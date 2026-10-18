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
- Sin una sesión de Seamless, el mod funciona igual que en un jugador.

## Requisitos (en las dos PCs)

1. **Elden Ring en Steam**, actualizado. Los dos tienen que ser amigos en Steam.
2. **Elden Ring Reforged (ERR)**, la **misma versión** en las dos PCs (Nexus Mods, mod 541).
   Se descomprime fuera de la carpeta del juego, por ejemplo en `C:\ERR`.
3. **Seamless Co-op**, la **misma versión** en las dos PCs (Nexus Mods, mod 510; probado con la 2.0.1).
4. **ER Mario, el mismo build** en las dos PCs: el mismo `er_mario.dll`, porque los paquetes llevan
   una versión.
   - Lo más fácil: el anfitrión le pasa al invitado su `er_mario.dll`.
   - La otra opción es compilarlo con el fork (sección *Building* del README principal).
5. **Una ROM de Super Mario 64 (US) propia** en cada PC (`.z64`, `.n64` o `.v64`).
   **La ROM no se comparte ni se sube a ningún lado.**

## Pasos en cada PC

### 1. Reforged

1. Descomprimir ERR (por ejemplo en `C:\ERR`) y abrirlo una vez con el acceso *Launch ELDEN RING
   Reforged (Offline)* para comprobar que arranca. Después cerrar el juego.
   - ✔ Existen las carpetas `C:\ERR\dll\offline` y `C:\ERR\addons`.
   - Si `addons` no existe, crearla. ER Mario la usa para detectar que está dentro de Reforged.

### 2. Seamless Co-op dentro de Reforged

1. Del zip de Seamless, copiar **solo** `SeamlessCoop\ersc.dll` y la carpeta `SeamlessCoop\locale`
   a `C:\ERR\dll\offline\`. Reforged ya trae su propio `ersc_settings.ini` en esa carpeta y se usa
   ese.
2. En `C:\ERR\dll\offline\ersc_settings.ini`, poner la **misma contraseña** en las dos PCs:
   `cooppassword = <la que elijan>`.
3. Anotar el valor de `save_file_extension` en ese mismo archivo (Seamless trae `co2`). Se usa en el
   paso 5.

### 3. ER Mario dentro de Reforged

1. Copiar a `C:\ERR\dll\offline\`:
   - `er_mario.dll`
   - la ROM propia de Super Mario 64 (US)
   - un `er_mario.ini` (se puede copiar el de una instalación de ER Mario y editarlo)
2. En `er_mario.ini`:
   - **PC de Luigi:** `character = luigi`
   - **PC de Mario:** sin esa línea, o `character = mario`
   - Si la ROM está en otro lado: `rom = C:\ruta\a\la\rom.z64`
3. Abrir con *Launch ELDEN RING Reforged (Offline)* hasta el menú principal y cerrar. **La primera vez
   hacen falta dos arranques:** en el primero el mod arma los modelos a partir de la ROM; en el
   segundo me3 ya los carga.
   - ✔ En `C:\ERR\dll\offline\logs\er_mario.log` aparece `package folder C:\ERR\addons\er-mario`.
   - ✔ Existe `C:\ERR\addons\er-mario\parts\bd_m_0996.partsbnd.dcx`, que es el modelo del compañero.

### 4. ¿Reforged trae sus propios archivos de menú, material o Torrent?

ER Mario reconstruye tres archivos del juego: los íconos del menú, la lista de materiales y Torrent
(que pasa a ser Yoshi). Si Reforged trae su propia versión de alguno, conviene que ER Mario parta de
la de Reforged, para no borrarle los cambios.

1. Buscar en la carpeta de Reforged, sin contar `addons\er-mario`:
   `menu\hi\01_common.tpf.dcx`, `material\allmaterial.matbinbnd.dcx`, `chr\c8002.chrbnd.dcx`.
2. Si aparece alguno, en `er_mario.ini` poner `base_assets = <carpeta que contiene menu\, material\ o chr\>`.
   Después borrar `C:\ERR\addons\er-mario\.built` y volver a arrancar dos veces.
   - ✔ En el log aparece `assets: /menu/hi/01_common.tpf.dcx from ...`.
3. Si no aparece ninguno, no hay que hacer nada.

### 5. La partida (solo en la PC de Mario, el anfitrión)

La idea es seguir **con la partida actual de Mario**, ahora de a dos. El progreso es del anfitrión.

1. Con el juego cerrado, ir a `%APPDATA%\EldenRing\<número de Steam>\`.
2. **Copia de seguridad:** copiar toda la carpeta a otro lado antes de tocar nada.
3. Ver qué archivo de partida usa Reforged con Seamless. Arrancar una vez *Reforged (Offline)* hasta
   el título, salir, y ver qué archivo se creó o modificó en esa carpeta. Debería ser
   `ER0000.<save_file_extension>` (por ejemplo `ER0000.co2`) o `ER0000.err`.
4. Reemplazar ese archivo por una **copia** de `ER0000_mario.sl2` (la partida de ER Mario en un
   jugador) con el nombre del paso anterior. El original `ER0000_mario.sl2` queda intacto para
   seguir jugando solo.
   - ✔ Al arrancar Reforged (Offline), *Continuar* carga el personaje de Mario donde estaba.
5. Reforged cambia armas, objetos y enemigos. Una partida vanilla carga, pero conviene revisar el
   equipo. El launcher de Reforged guarda hasta 25 copias de la partida por su cuenta.

La PC de Luigi puede crear un personaje nuevo en su propia partida de Reforged. El progreso del
mundo es del anfitrión.

### 6. Jugar

1. Los dos abren *Launch ELDEN RING Reforged (Offline)* y cargan su personaje.
2. Mario (anfitrión) usa la **Tiny Great Pot** (Gran olla pequeña). Luigi usa el **Effigy of Malenia**
   (Efigie de Malenia) para unirse. Las dos cosas las da Seamless.
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
| `coop: packets of version N` | Tienen versiones distintas de ER Mario |
| `coop: partner's character paired by elimination` | Normal si hay un solo compañero: el personaje no traía el SteamID donde se esperaba |
| `combat: a partner's ... not here` | Un golpe del invitado sobre un enemigo que el anfitrión no tiene cargado (se ignora) |
| `combat: attack 99X as found: ... LOOKS USED` | Reforged usa esa fila de ataque: avisar, porque hay que mover las filas del mod |
| `equip: armour rows as found` | Modelos originales de las pecheras 660100 y 661100 en Reforged |

## Perfil de me3 sin Reforged

Para jugar en cooperativo con el juego vanilla (sin Reforged), está `docs/coop/er-mario-coop.me3`.
Carga `SeamlessCoop/ersc.dll` y `er_mario.dll`. Los dos jugadores tienen que usar lo mismo:
Reforged los dos, o vanilla los dos.
