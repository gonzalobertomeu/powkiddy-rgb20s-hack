# Groovebox — spec de interfaz

Última revisión: 2026-11-20 (H6: archivo en Proyecto, navegador de proyectos, editor de nombres y diálogos implementados) · Estado: borrador para revisión del usuario · Volver al [índice](README.md)

Decisiones base: [ADR 0002 — interfaz híbrida grilla + tracker](../../decisions/0002-interfaz-hibrida-grilla-tracker.md) (aceptada) ·
[ADR 0006 — cadenas por pista](../../decisions/0006-cadenas-por-pista.md) (aceptada) ·
[ADR 0007 — polifonía en el MVP](../../decisions/0007-polifonia-en-el-mvp.md) (aceptada).

## Convenciones de los mockups

- Cada mockup es una grilla de **80 × 30 caracteres**, que aproxima 640×480 con una fuente de 8×16 px.
- Los gráficos reales (bloques de la grilla, medidores, forma de onda) se dibujan con primitivas de SDL2, no con caracteres. El mockup indica la ubicación.
- Símbolos: `#` paso con nota · `&` paso con acorde · `-` la nota sigue sonando (largo) · `x` OFF (libera las voces) · `.` paso vacío · `[ ]` cursor · `>` posición de reproducción · `^` columna que suena · `M`/`S` mute/solo · `---` celda vacía · `*` frase o cadena usada en más de un lugar · `!` aviso.
- Números de frase y de cadena: 3 dígitos decimales, `000`–`255`.
- Lo marcado **(v1)** se dibuja deshabilitado o no aparece en el MVP.

Regla de 80 columnas, para revisar los mockups:

```
0         1         2         3         4         5         6         7
01234567890123456789012345678901234567890123456789012345678901234567890123456789
```

## Principios de control

1. **Un solo modificador de edición: A.** Tocar A inserta o activa; **mantener A + cruz** cambia el valor bajo el cursor:
   **←/→ = paso fino** (±1) y **↑/↓ = paso grueso** (±12 en notas, ±10 en valores de 0–127 y en números de frase o cadena, ±1 en listas).
2. **Un solo modificador global: Select.** Select + cruz cambia de pantalla; Select + otro botón ejecuta un atajo global.
3. **B = volver o cancelar.** Mantener B + cruz hace saltos grandes del cursor. **A + B** (en cualquier orden) borra.
4. **X = escuchar** lo que hay bajo el cursor, aunque el secuenciador esté parado.
5. **Y = seleccionar** (copiar, cortar y pegar). **A + Y = clonar**: copia la frase, cadena o instrumento bajo el cursor a un número libre y pone la copia en ese lugar.
6. **Doble toque de A** sobre una celda de número vacía (frase o cadena) crea un número **nuevo y vacío**. Un toque simple inserta el último número usado.
7. **A + L1/R1 = acorde**: sobre una nota, recorre los tipos de acorde.
8. **Gatillos:** L1/R1 cambian de pista; L2/R2 se mueven por el **nivel de arriba** (sección, frase de la cadena o fila de la canción, según la pantalla).
9. **Start = reproducir/parar** lo que corresponde a la pantalla; **Select + Start = loop de la sección actual** con todas las pistas.
10. **Sticks = performance**: mueven parámetros en vivo y vuelven al valor guardado al soltarlos (no se graban en el MVP).
11. Nada importante necesita más de dos botones a la vez.
12. **No se usan** el botón FN, los botones de volumen ni L3/R3 (clic de los sticks): pueden estar reservados por el sistema o no existir **(sin verificar)**.

Repetición de teclas: 250 ms de espera y luego una repetición cada 33 ms. Con A + cruz mantenido más de 1 s, el paso se multiplica por 4.
Doble toque: dos pulsaciones de A en menos de 300 ms sin mover la cruz.
Los nombres A, B, X e Y son las etiquetas de la consola; el mapeo físico se configura en `config.ron` ([spec-formatos](spec-formatos.md#configuración-de-la-app)).

## Modos

| Modo | Cómo se entra | Cómo se sale | Qué cambia |
|---|---|---|---|
| Normal | — | — | La cruz mueve el cursor |
| Edición | Mantener A | Soltar A | La cruz cambia el valor bajo el cursor; L1/R1 cambian el acorde; Y clona; B borra |
| Selección | Y | A (copiar), X (cortar), B (cancelar) | La cruz extiende la selección |
| Modal | Abrir el navegador, el editor de nombres o un diálogo | A (aceptar) o B (cancelar) | Select + cruz y los gatillos se desactivan |
| Reproducción | Start o Select + Start | Start | Ortogonal a los anteriores: se puede editar todo mientras suena |

## Contexto de edición: la sección

Las pantallas comparten un **contexto**: la pista actual `t`, la fila de canción `R` y la entrada de cadena `k`.
La pareja `(R, k)` es la **sección**: para cada pista, la frase que tiene en la entrada `k` de su cadena de la fila `R`.

- **Canción** fija `R` (fila del cursor) y `t` (columna del cursor).
- **Grilla** muestra la sección `(R, k)` de las 8 pistas; L2/R2 cambian `k` (y pasan a la fila siguiente o anterior al terminar las cadenas de la fila).
- **Cadena** muestra la cadena de la pista `t` en la fila `R`; el cursor fija `k`.
- **Frase** muestra la frase de la pista `t` en la sección `(R, k)`.
- **Instrumento** muestra el instrumento de la pista `t`.

Con la opción "seguir reproducción" (en Proyecto), la Grilla y la Frase siguen a los cabezales mientras suena.

**Implementado (H5):** la opción está **activada por defecto** (desvío: el mockup de Proyecto dice "NO"; con Start en la
Grilla tocando la canción, sin seguir el cabezal desaparece al pasar de sección). La sección salta a la del cabezal de
la pista actual cuando ese cabezal cambia de sección, al cambiar de pista y al entrar en la Grilla o la Frase; entre
medio, L2/R2 pueden mirar otra sección. En la Grilla, cada pista muestra la frase donde está su cabezal (FRA en verde) y
una línea del panel lista las pistas que están en otra sección ("en otra sección: PALMA fila 00 sec. 2").

## Mapa de pantallas

Se navega con **Select + cruz**. La fila del medio va de lo general (izquierda) a lo particular (derecha).
El minimapa (abajo a la derecha) muestra la pantalla actual entre corchetes.

```
                                PROYECTO
                                    ^
   CANCIÓN  <-->  GRILLA  <-->  CADENA  <-->  FRASE  <-->  INSTRUMENTO
                                    v
                                 MEZCLA
```

- **Canción** y **Grilla** muestran las 8 pistas; **Cadena**, **Frase** e **Instrumento**, una sola (L1/R1 cambian de pista).
- **Proyecto** y **Mezcla** se alcanzan con Select + ↑ y Select + ↓ desde **cualquier** pantalla de la fila del medio; Select + ↓ (o ↑) vuelve a la pantalla de origen.
- El **Navegador**, el **Editor de nombres** y los **Diálogos** son modales: se abren desde una acción y vuelven a la pantalla que los abrió.
- Minimapa: `PRY` arriba, `CAN GRI CAD FRA INS` en el medio, `MEZ` abajo.

## Mapa global de controles

Vale en todas las pantallas, salvo que la tabla de cada pantalla diga otra cosa.

| Control | Solo | + A (mantener) | + B (mantener) | + Select |
|---|---|---|---|---|
| Cruz | Mover el cursor | Cambiar el valor (←/→ fino, ↑/↓ grueso) | Salto grande del cursor (según pantalla) | Cambiar de pantalla |
| A | Insertar el último valor / activar / abrir. Doble toque en número vacío: número nuevo | — | **Borrar** (A + B) | **Pegar** |
| B | Cancelar, cerrar modal o salir de la selección | **Borrar** (A + B) | — | Deshacer **(v1)** |
| X | Escuchar (audición) | — | — | **Mute** de la pista actual |
| Y | Entrar en modo selección | **Clonar** la frase, cadena o instrumento bajo el cursor | — | **Solo** de la pista actual |
| L1 / R1 | Pista anterior / siguiente | **Acorde** − / + (sobre una nota) | — | Octava base − / + (para notas nuevas) |
| L2 / R2 | Anterior / siguiente en el nivel de arriba (ver cada pantalla) | — | — | Tempo − / + 1 BPM |
| Start | Reproducir / parar según la pantalla (tabla siguiente) | — | — | **Loop de la sección** actual con las 8 pistas |
| Select (sola) | Mientras se mantiene, el minimapa se agranda. **(v1)**: mantener 1 s abre la ayuda | | | |
| Stick izquierdo | X: corte del filtro de la pista actual; Y: resonancia | | | |
| Stick derecho | X: envío a delay de la pista actual; Y: envío a reverb | | | |

**Qué reproduce Start** ([modos del motor](spec-motor.md#modos-de-reproducción)):

| Pantalla | Start | Select + Start |
|---|---|---|
| Canción, Grilla, Mezcla, Proyecto | La canción, todas las pistas, desde la fila `R` | Loop de la sección `(R, k)` |
| Cadena | Loop de la cadena, solo la pista actual | Loop de la sección |
| Frase, Instrumento | Loop de la frase, solo la pista actual | Loop de la sección |

Con algo sonando, Start detiene. Cambiar de pantalla no cambia lo que suena.

**Implementado (H5)** tal cual la tabla; Select + Start con algo sonando también detiene (la spec no lo define) y
mantener Select + Start 1 s sale de la app. El título dice qué suena (`> CANCIÓN`, `> LOOP SEC`, `> LOOP CAD`,
`> LOOP FRA`) y el pie empieza con Start y `Sel+Start loop`, para que entren con la fuente grande (2026-11-21).

Detalles de los sticks: zona muerta del 15 % (configurable); el desplazamiento se suma al valor guardado y se suaviza; al soltar,
vuelve al valor guardado en unos 100 ms. Arriba es positivo. La calibración está en `config.ron`.

## Pantallas

### 1. Canción

**Propósito:** el arreglo. Cada fila tiene, para cada pista, una cadena. Cada pista avanza por su columna a su propio ritmo.

```
CANCIÓN                                     > 120.0 BPM  fila 01/256  beat_01*
--------------------------------------------------------------------------------
 FILA  BOMBO CAJA  HH-CE HH-AB PALMA SAMPL BAJO  SINTE   LARGO DE CADENAS
  00   000   001   002   003   004   ---   006   007     4 4 4 4 4 - 4 4
> 01  [000]  001   002   008   004   005   009   010     4 4 4 4 4 1 4 2  !
  02   011   001   002   003   004   ---   006   007     4 4 4 4 4 - 4 4
  03   ---   ---   ---   ---   ---   ---   ---   ---
  04   ---   ---   ---   ---   ---   ---   ---   ---
  05   012   013   002   ---   ---   ---   014   015     2 2 4 - - - 4 4  !
  06   ---   ---   ---   ---   ---   ---   ---   ---
  07   ---   ---   ---   ---   ---   ---   ---   ---
  08   ---   ---   ---   ---   ---   ---   ---   ---
  09   ---   ---   ---   ---   ---   ---   ---   ---
  10   ---   ---   ---   ---   ---   ---   ---   ---
  11   ---   ---   ---   ---   ---   ---   ---   ---
  12   ---   ---   ---   ---   ---   ---   ---   ---
  13   ---   ---   ---   ---   ---   ---   ---   ---
  14   ---   ---   ---   ---   ---   ---   ---   ---
  15   ---   ---   ---   ---   ---   ---   ---   ---
--------------------------------------------------------------------------------
 fila 01: SAMPL dura 1 frase y SINTE 2; las demás 4. Las pistas se desalinean.
 Filas 00-02 y 05 son bloques separados: cada pista vuelve al inicio de su bloque.
 CADENA 000 (usada 2 veces)                     duración del bloque: 0:24
--------------------------------------------------------------------------------
 A poner  2xA nueva  A+Y clonar  A+B borrar  Y selec.               PRY
 L1/R1 pista  L2/R2 página  Start canción  Sel+Start loop  [CAN]GRI CAD FRA INS
                                                                    MEZ
```

Notas de dibujo:
- La celda que suena en cada pista se resalta (cada pista puede estar en una fila distinta).
- La columna "LARGO DE CADENAS" da cuántas frases tiene cada cadena; `!` marca filas con largos distintos.

| Control | Solo | + A | + B | + Select |
|---|---|---|---|---|
| Cruz | Mover por fila y pista | Número de cadena ±1 / ±10 | ↑/↓: 16 filas; ←/→: primera / última pista | Cambiar de pantalla |
| A | Celda vacía: insertar la última cadena usada. Doble toque: cadena nueva vacía | — | Vaciar la celda | Pegar |
| X | Escuchar la primera frase de la cadena (una vez) | — | — | Mute de la pista |
| Y | Selección (celda → fila → bloque) | Clonar la cadena (copia nueva en esta celda) | — | Solo de la pista |
| L1 / R1 | Pista − / + | — | — | Octava − / + |
| L2 / R2 | Página de 16 filas − / + | — | — | Tempo − / + |

Transiciones: Select + → Grilla (sección `(R, 0)`, cursor en la pista `t`) · Select + ↑ Proyecto · Select + ↓ Mezcla.

**Implementado (H5)**, con estos detalles: la fila siguiente a la última se puede editar (crea la fila); A en una celda
vacía sin "última cadena" (recién abierto el proyecto) pone una cadena nueva y el doble toque inmediato se queda con esa;
A sobre una celda con cadena la toma como "última"; A + cruz sobre un número que no existe crea esa cadena vacía. La
celda que suena se dibuja en verde con una barra de avance por la cadena y `>` marca las filas con algún cabezal. La
columna LARGO va con la fuente chica (entra con las dos fuentes); con 12×24 los nombres de pista quedan en 3 letras.
**X** toca la primera frase de la cadena entera, **una vez**, con su transposición (H4: vista previa del motor,
[spec-motor](spec-motor.md#modos-de-reproducción)); si la frase no tiene notas, suena el instrumento. La selección (Y) y
pegar están implementados desde el H10 (ver [Modo selección](#modo-selección)).

### 2. Grilla

**Propósito:** armar ritmos rápido. Muestra las 8 pistas en la sección actual: la frase de 16 pasos que cada pista tiene ahí.

```
GRILLA  fila 00  sección 1/4               > 120.0 BPM  sw 54%   beat_01*
--------------------------------------------------------------------------------
 # PISTA  FRA  1   2   3   4   5   6   7   8   9  10  11  12  13  14  15  16

 1 BOMBO  000 [#]  .   .   .   #   .   .   .   #   .   .   .   #   .   .   .

 2 CAJA   001  .   .   .   .   #   .   .   .   .   .   .   .   #   .   .   .

 3 HH-CER 002  #   .   #   .   #   .   #   .   #   .   #   .   #   .   #   .

 4 HH-ABI 003  .   .   .   .   .   .   .   .   .   .   .   .   .   .   #   .

 5 PALMAS 004  .   .   .   .   .   .   .   .   .   .   .   .   #   .   .   .

 6 SAMPLE ---  (sin frase en esta sección: A en un paso crea cadena y frase)

 7 BAJO   006  #   .   .   #   .   .   #   .   .   .   #   .   .   .   #   .  M

 8 SINTE  007  &   -   -   -   &   -   -   -   &   -   -   -   x   .   .   .
                                   ^
--------------------------------------------------------------------------------
 PISTA 1 BOMBO  cadena 000  frase 000 (usada 4 veces)   PASO 01  C-4  vel 100
 voces de la pista 1   bolsa 9/16                stick izq: filtro  der: fx
--------------------------------------------------------------------------------
 A poner/editar  A+L1/R1 acorde  A+B borrar  X escuchar  Y selec.   PRY
 L1/R1 pista  L2/R2 sección  Start canción  Sel+Start loop  CAN[GRI]CAD FRA INS
                                                                  MEZ
```

Notas de dibujo:
- Cada paso se dibuja como un bloque de unos 24×20 px (color según el tipo de pista), con más intensidad cuanto mayor la velocidad. Las semicorcheas 1, 5, 9 y 13 llevan una marca de tiempo fuerte.
- Los pasos con acorde (`&`) se dibujan con varias rayas horizontales; el largo de la nota (`-`) se dibuja como una barra que continúa. Así se ve la superposición de las notas en pistas polifónicas.
- La columna que suena se resalta en toda la altura. Si "seguir reproducción" está activo y las pistas están en secciones distintas, cada fila muestra la frase que suena en esa pista y su cabezal propio.
- Los pasos con p-lock **(v1)** llevan un punto en una esquina; los que tienen condición **(v1)**, un borde punteado. Una pista en mute se dibuja atenuada.
- La columna **FRA** es editable: muestra el número de frase de la pista en esta sección.

**Creación implícita:** si una pista no tiene frase en la sección (no tiene cadena en la fila `R`, o su cadena termina antes de `k`),
poner una nota crea lo que falte: una cadena nueva en la celda `(R, t)` de la canción y/o una frase nueva en la entrada `k`.
Solo se crea en la primera entrada libre de la cadena; si hay un hueco, se avisa ("la cadena de esta pista termina en la sección 2").

| Control | Solo | + A | + B | + Select |
|---|---|---|---|---|
| Cruz | Mover por pista y paso; ← desde el paso 1 entra en la columna FRA | En un paso: nota ±1 / ±12 (suena al cambiar). En FRA: número de frase ±1 / ±10 | ↑/↓: primera / última pista; ←/→: paso 1 / paso 16 | Cambiar de pantalla |
| A | Paso vacío: poner nota (última nota, acorde y velocidad de la pista). En FRA vacía: última frase usada; doble toque: frase nueva | — | En un paso: borrarlo. En FRA: quitar la frase de la cadena (las siguientes suben) | Pegar |
| X | Escuchar el paso (o el instrumento si está vacío) | — | — | Mute de la pista |
| Y | Selección (celda → pista → sección completa) | En FRA: clonar la frase y poner la copia en este lugar | — | Solo de la pista |
| L1 / R1 | Pista − / + (igual que ↑/↓) | Acorde del paso − / + | — | Octava − / + |
| L2 / R2 | Sección − / + (`k`; al pasar el final de las cadenas de la fila, sigue en la fila siguiente) | — | — | Tempo − / + |

Transiciones: Select + ← Canción · Select + → Cadena (de la pista bajo el cursor, con el cursor en `k`) · Select + ↑ Proyecto · Select + ↓ Mezcla.

### 3. Cadena

**Propósito:** la lista de frases de una pista en una fila de la canción, con transposición por entrada.

```
CADENA 007  PISTA 8 SINTE  fila 00  (usada 1 vez)   > 120.0 BPM     beat_01*
--------------------------------------------------------------------------------
  POS  FRASE  TRANSP.             FRASE 012 (vista previa)
   01   007     +00                01 C-4  men7/1      09 A#3  dom7
>  02   007     +05                02 ---              10 ---
   03  [012]    +00                03 ---              11 ---
   04   013*    +00                04 ---              12 ---
   05   ---     ---                05 G#3  may         13 OFF
   06   ---     ---                06 ---              14 ---
   07   ---     ---                07 ---              15 ---
   08   ---     ---                08 ---              16 ---
   09   ---     ---
   10   ---     ---
   11   ---     ---
   12   ---     ---
   13   ---     ---
   14   ---     ---
   15   ---     ---
   16   ---     ---
--------------------------------------------------------------------------------
 La cadena termina en la primera posición vacía (aquí: 4 frases = 4 compases).
 FRASE 012   A+izq/der: ±1   A+arr/abj: ±10   2xA en vacía: frase nueva
--------------------------------------------------------------------------------
 A poner  2xA nueva  A+Y clonar  A+B borrar  X escuchar  Y selec.   PRY
 L1/R1 pista  L2/R2 fila  Start cadena  Sel+Start loop  CAN GRI[CAD]FRA INS
                                                                  MEZ
```

| Control | Solo | + A | + B | + Select |
|---|---|---|---|---|
| Cruz | Mover por posición y columna | FRASE: ±1 / ±10; TRANSP.: ±1 / ±12 | ↑/↓: primera / última posición | Cambiar de pantalla |
| A | FRASE vacía: insertar la última frase usada. Doble toque: frase nueva vacía | — | Quitar la entrada (las siguientes suben) | Pegar |
| X | Escuchar la frase de la posición (una vez) | — | — | Mute de la pista |
| Y | Selección de posiciones | Clonar la frase de esta posición | — | Solo de la pista |
| L1 / R1 | Pista − / + (la cadena de esa pista en la misma fila) | — | — | Octava − / + |
| L2 / R2 | Fila anterior / siguiente de la canción (la cadena de esta pista en esa fila) | — | — | Tempo − / + |
| Start | Loop de esta cadena, solo esta pista | — | — | Loop de la sección |

Transiciones: Select + ← Grilla · Select + → Frase (la de la posición bajo el cursor) · Select + ↑ Proyecto · Select + ↓ Mezcla.

**Implementado (H5)**: cursor en dos columnas (FRASE y TRANSP.), cabezal (`>` en la posición y barra de avance), vista
previa de la frase bajo el cursor (solo con 8×16). En una fila sin cadena, A en FRASE crea la cadena y la frase
(creación implícita, como en la Grilla); una posición después de un hueco avisa. X: la frase de la posición entera, una
vez, con la transposición (H4, como en la Canción).

### 4. Frase (tracker)

**Propósito:** el detalle de una frase, paso por paso, con todas sus columnas. Mismos datos que una fila de la Grilla.

```
FRASE 007  PISTA 8 SINTE  cadena 007 pos 01  fila 00   > 120.0 BPM  beat_01*
--------------------------------------------------------------------------------
  PASO  NOTA  ACORDE  VEL    LARGO LOCK 1    LOCK 2    COND   MICRO REP
   01   C-4   men7/1  090    8     ---  ---  ---  ---  ---    +00   1
   02   ---   ---     ---    ---   ---  ---  ---  ---  ---    ---   -
   03   ---   ---     ---    ---   ---  ---  ---  ---  ---    ---   -
   04   ---   ---     ---    ---   ---  ---  ---  ---  ---    ---   -
   05   G#3  [may   ] 080    4     ---  ---  ---  ---  ---    +00   1
   06   ---   ---     ---    ---   ---  ---  ---  ---  ---    ---   -
   07   ---   ---     ---    ---   ---  ---  ---  ---  ---    ---   -
   08   ---   ---     ---    ---   ---  ---  ---  ---  ---    ---   -
>  09   A#3   dom7    090    8     CORT 080  ---  ---  1:2    +00   1
   10   ---   ---     ---    ---   ---  ---  ---  ---  ---    ---   -
   11   ---   ---     ---    ---   ---  ---  ---  ---  ---    ---   -
   12   ---   ---     ---    ---   ---  ---  ---  ---  ---    ---   -
   13   OFF   ---     ---    ---   ---  ---  ---  ---  ---    ---   -
   14   ---   ---     ---    ---   ---  ---  ---  ---  ---    ---   -
   15   ---   ---     ---    ---   ---  ---  ---  ---  ---    ---   -
   16   ---   ---     ---    ---   ---  ---  ---  ---  ---    ---   -
--------------------------------------------------------------------------------
 ACORDE may: G#3 C-4 D#4, inversión 0    A+izq/der: tipo   A+arr/abj: inversión
 voces de la pista 4   bolsa 9/16   MVP: NOTA ACORDE VEL LARGO   v1: LOCK..REP
--------------------------------------------------------------------------------
 A poner/editar  A+L1/R1 acorde  A+B borrar  A+Y clonar  X escuchar  PRY
 L1/R1 pista  L2/R2 frase  Start frase  Sel+Start loop    CAN GRI CAD[FRA]INS
                                                                  MEZ
```

Columnas:

| Columna | Rango | Prioridad | Notas |
|---|---|---|---|
| NOTA | C-0 a G-9 (MIDI 12–127), `---` vacío, `OFF` libera todas las voces de la pista | MVP | Insertar usa la última nota de la pista |
| ACORDE | `---`, `5`, `8va`, `may`, `men`, `dis`, `aum`, `sus2`, `sus4`, `may7`, `men7`, `dom7`, `m7b5`, `add9`; inversión `/1`–`/3` | MVP | Hasta 4 notas. Se ignora en percusión. Si la pista tiene menos voces, se descartan las notas más agudas |
| VEL | 1–127 | MVP | |
| LARGO | 1/4, 1/2, 3/4, 1 a 16 pasos, `INF` (hasta la próxima nota de la pista) | MVP | En ticks internamente ([spec-motor](spec-motor.md#secuenciador)). Con más de 1 voz, las notas largas se superponen |
| LOCK 1 / LOCK 2 | Parámetro (abreviatura de 4 letras) + valor 0–127 | v1 | En la parte del parámetro, A + cruz recorre la lista de parámetros |
| COND | `---`, `10%`…`90%`, `1:2`…`8:8` (A de B), `FILL`, `!FIL`, `1RA`, `!1RA`, `PRE`, `!PRE` | v1 | Ver [spec-motor](spec-motor.md#condiciones-de-paso-v1) |
| MICRO | −12 a +12 ticks (24 ticks = 1 paso) | v1 | |
| REP | 1–8 repeticiones dentro del largo del paso | v1 | Con caída de velocidad configurable |

**Cómo se escribe un acorde con la cruz:**
1. Poner la nota raíz (A en NOTA; A + ←/→ y A + ↑/↓ para elegirla).
2. Sin soltar A, **L1/R1** recorren los tipos de acorde (suena cada vez). También se puede ir a la columna ACORDE: A + ←/→ cambia el tipo y A + ↑/↓ la inversión.
3. El paso siguiente que se inserte con A repite la nota, el acorde y la velocidad usados por última vez en la pista, así una progresión se escribe moviendo solo la raíz.
4. **(v1):** acorde libre (3 intervalos editables) y rasgueo.

| Control | Solo | + A | + B | + Select |
|---|---|---|---|---|
| Cruz | Mover por fila y columna | Cambiar el valor de la celda (en ACORDE: ←/→ tipo, ↑/↓ inversión) | ↑/↓: fila 1 / fila 16; ←/→: primera / última columna | Cambiar de pantalla |
| A | Celda vacía: insertar el último valor usado en esa columna | — | Borrar la celda (en NOTA borra todo el paso) | Pegar |
| X | Escuchar la nota (o el acorde) de la fila | — | — | Mute de la pista |
| Y | Selección (celda → columna → frase completa) | Clonar esta frase (la copia reemplaza a la original en esta posición de la cadena) | — | Solo de la pista |
| L1 / R1 | Pista − / + (la frase de esa pista en la misma sección) | Acorde − / + | — | Octava − / + |
| L2 / R2 | Frase anterior / siguiente de la cadena | — | — | Tempo − / + |
| Start | Loop de esta frase, solo esta pista | — | — | Loop de la sección |

Si la pista no tiene frase en la sección, la pantalla muestra "sin frase" y poner una nota la crea (igual que en la Grilla).

Transiciones: Select + ← Cadena · Select + → Instrumento de esta pista · Select + ↑ Proyecto · Select + ↓ Mezcla.

### 5. Instrumento

**Propósito:** diseñar el sonido de la pista actual. El contenido depende del tipo de instrumento. Los parámetros se ordenan en hasta 3 columnas por grupos.

**Sustractivo:**

```
INSTRUMENTO  PISTA 8 SINTE         tipo: [SUSTRACTIVO]                beat_01*
--------------------------------------------------------------------------------
 OSCILADOR                  FILTRO                     ENVOLVENTE AMP
   forma      SIERRA          tipo      PASA-BAJOS       ataque       002
   afinación  +00 st          corte    [064]             decaimiento  040
   fino       +00 ct          resonan.  030              sostén       100
   pulso      064             env. amt  +050             relajación   020
   sub        040             seguim.   064
   ruido      000             drive     000 (v1)       ENVOLVENTE FILTRO
                                                          ataque       000
 GENERAL                    LFO (v1)                      decaimiento  030
   volumen    100             forma     TRI               sostén       000
   voces      4               veloc.    040               relajación   010
   veloc>amp  064             destino   CORTE
   glide      000 (v1)        cantidad  000

 +-- filtro --------------------------+  +-- env. amp ---------------------+
 |\___                                |  |  /\                             |
 |    \___                            |  |_/  \__________                  |
 |        \____________               |  |               \___             |
 +------------------------------------+  +---------------------------------+
--------------------------------------------------------------------------------
 CORTE DEL FILTRO  064  (aprox. 650 Hz)      A+cruz: cambiar   X: escuchar C-3
 A poner/editar  A+B valor por defecto  A+Y clonar  Y copiar instr.   PRY
 L1/R1 pista  Start frase  Sel+Start loop sección     CAN GRI CAD FRA[INS]
                                                                  MEZ
```

**Sample:**

```
INSTRUMENTO  PISTA 6 SAMPLE            tipo: [SAMPLE]                 beat_01*
--------------------------------------------------------------------------------
 archivo  [vox_hey.wav]        0,82 s   mono   44,1 kHz   16 bit
 +----------------------------------------------------------------------------+
 |         .:|||:.                                                            |
 |  [  .:|||||||||||:..        ..::..                              ]          |
 |         ':|||:'                                                            |
 +----------------------------------------------------------------------------+
    ^ inicio                                                       ^ fin
 REPRODUCCIÓN               FILTRO                     ENVOLVENTE AMP
   afinación  +00 st          tipo      PASA-BAJOS       ataque       000
   fino       +00 ct          corte     127              decaimiento  080
   inicio     000             resonan.  000
   fin        127             env. amt  +000           GENERAL
   loop       NO (v1)                                     volumen      100
   reversa    NO (v1)                                     voces        2
   slices     -- (v1)                                     veloc>amp    064
--------------------------------------------------------------------------------
 A sobre "archivo": abrir el navegador     X: escuchar     A+B: valor por def.
 L1/R1 pista  Start frase  Sel+Start loop sección     CAN GRI CAD FRA[INS]
                                                                  MEZ
```

**Percusión sintetizada** (sin mockup propio, mismo layout de columnas):

| Grupo | Parámetros |
|---|---|
| Modelo | BOMBO, CAJA, HAT-CERRADO, HAT-ABIERTO, PALMAS, TOM |
| Tono | afinación, barrido (cantidad y tiempo, en bombo y tom) |
| Cuerpo | decaimiento, golpe (click), color (mezcla tono/ruido) |
| Filtro | corte, resonancia |
| General | volumen, veloc>amp, voces (por defecto 1) |

**FM de 2 operadores (v1):** relación del modulador, índice, envolvente del índice, realimentación, envolvente de amplitud, voces.

| Control | Solo | + A | + B | + Select |
|---|---|---|---|---|
| Cruz | Mover entre parámetros (en 2D) | Cambiar el valor (en listas, ←/→ y ↑/↓ recorren) | ↑/↓: primer / último parámetro de la columna; ←/→: columna | Cambiar de pantalla |
| A | Sobre "archivo": abrir el Navegador. Sobre "tipo": pedir confirmación para cambiarlo | — | Restaurar el valor por defecto del parámetro | Pegar el instrumento copiado |
| X | Escuchar el instrumento (última nota y acorde usados, o C-4) | — | — | Mute de la pista |
| Y | Seleccionar el instrumento completo (luego A copia) | Clonar a preset **(v1)** | — | Solo de la pista |
| L1 / R1 | Pista − / + | — | — | Octava − / + |
| L2 / R2 | Sin función | — | — | Tempo − / + |

Cambiar el **tipo** abre un diálogo ("Cambiar el tipo borra los parámetros de esta pista. A: confirmar, B: cancelar"), porque el MVP no tiene deshacer.

Transiciones: Select + ← Frase · Select + ↑ Proyecto · Select + ↓ Mezcla · A sobre "archivo" → Navegador.

**Implementado (H4), parcial:** la pantalla muestra el tipo de la pista y un solo parámetro, **voces** (GENERAL), con
el cursor fijo en él: A + cruz ±1 (1–8), A + B vuelve al valor por defecto (el del kit para esa pista si sigue con el
mismo tipo; si no, el del tipo), X escucha, L1/R1 cambian de pista y Start toca el loop de la frase. Debajo, una
explicación de mono / poli y del robo. Los demás parámetros, la navegación en 2D y el cambio de tipo llegan en H8.

### 6. Mezcla

**Propósito:** balance, envíos y efectos globales.

```
MEZCLA                                               > 120.0 BPM      beat_01*
--------------------------------------------------------------------------------
         1 BOMBO 2 CAJA  3 HH-CE 4 HH-AB 5 PALMA 6 SAMPL 7 BAJO  8 SINTE MASTER
 nivel   ######  ####    ##      ###     #       .       #####   ###     ######
 voces   1/1     1/1     1/1     1/1     0/1     0/2     1/1     4/4     9/16
 volumen [100]   090     070     085     100     080     095     075     100
 paneo    C       C      R20     L10      C      L30      C      R15
 delay    000     010    000     020     040     000     000     030
 reverb   000     020    005     030     060     010     000     025
 mute     -       -       -       -       -       M       -       -
 solo     -       -       -       -       -       -       -       -

--------------------------------------------------------------------------------
 DELAY    tiempo 3/16    realim. 040    tono 090    ancho 100    retorno 080
 REVERB   tamaño 060     amort.  050    predelay 010             retorno 070
 MASTER   limitador SÍ   ganancia +0 dB
--------------------------------------------------------------------------------
 VOLUMEN PISTA 1  100                                              PRY
 A+cruz: cambiar   A en mute/solo: alternar   X: escuchar  CAN GRI CAD FRA INS
 A+B: valor por defecto                                           [MEZ]
```

Los medidores de "nivel" son barras dibujadas (pico con caída) y la fila "voces" muestra las voces en uso sobre el límite de cada pista
(y de la bolsa, en MASTER), alimentadas por eventos del hilo de audio.

| Control | Solo | + A | + B | + Select |
|---|---|---|---|---|
| Cruz | Mover por canal y fila (incluye las filas de efectos) | Cambiar el valor | ←/→: primer canal / master; ↑/↓: primera / última fila | Select + ↑: volver a la pantalla de origen |
| A | En mute o solo: alternar | — | Valor por defecto | — |
| X | Escuchar el instrumento del canal | — | — | Mute del canal bajo el cursor |
| Y | — (sin selección en el MVP) | — | — | Solo del canal bajo el cursor |
| L1 / R1 | Canal − / + | — | — | Octava − / + |
| L2 / R2 | Sin función | — | — | Tempo − / + |

### 7. Proyecto

**Propósito:** archivo, tempo y ajustes.

```
PROYECTO                                                              beat_01*
--------------------------------------------------------------------------------
  nombre         [beat_01]
  tempo           120.0 BPM
  swing           54 %
  octava base     4

  > GUARDAR
    GUARDAR COMO...
    CARGAR...
    NUEVO
    EXPORTAR WAV... (v1)

  AJUSTES
    audición al editar      SÍ
    seguir reproducción     NO
    zona muerta sticks      15 %
    brillo                  80 % (sin verificar que se pueda controlar)

  ESTADO
    audio 48000 Hz  buffer 256 (5,3 ms)  CPU audio 17 %  xruns 0
    voces 9/16 (máx. 14)   frases 15/256   cadenas 16/256   samples 3,2/128 MB
    batería 78 %   versión 0.1.0
    SALIR
--------------------------------------------------------------------------------
 A: ejecutar / editar   A+cruz: cambiar valor   B: volver             [PRY]
                                                      CAN GRI CAD FRA INS
                                                                  MEZ
```

| Control | Solo | + A | + B | + Select |
|---|---|---|---|---|
| Cruz | Mover por campos y acciones | Cambiar el valor (tempo: ±0,1 / ±1 BPM; swing: ±1 / ±5 %) | — | Select + ↓: volver a la pantalla de origen |
| A | Ejecutar la acción; sobre "nombre", abrir el Editor de nombres | — | — | — |
| B | Volver a la pantalla de origen | — | — | — |
| Start | Reproducir / parar la canción | — | — | Loop de la sección |

"SALIR" y "CARGAR" piden confirmación si hay cambios sin guardar (`*` junto al nombre del proyecto).

**Implementado (H6)**, con estos detalles:

- El cursor arranca en **GUARDAR**. "nombre" es una fila más del cursor: A abre el editor de nombres para
  **renombrar** (si el proyecto ya está guardado, se renombra su carpeta; no guarda los cambios pendientes).
  **GUARDAR COMO...** abre el mismo editor y guarda una **copia** con el nombre nuevo, que pasa a ser el proyecto
  abierto (la carpeta anterior queda como estaba). Si el nombre ya existe, pide confirmación para reemplazarlo (su
  versión anterior queda en el `.bak`).
- **NUEVO** también pide confirmación si hay cambios sin guardar (la spec solo lo pide para SALIR y CARGAR). El
  proyecto nuevo se llama `proyecto-NNN` (el primer número sin carpeta) y se puede renombrar.
- Junto al nombre: `* sin guardar` o `solo lectura` (archivo de un formato más nuevo que la app). En el título, `*`
  como en todas las pantallas.
- **Tempo y swing (H4)** son filas del cursor debajo de "nombre": A + ←/→ ±0,1 BPM y ±1 %, A + ↑/↓ ±1 BPM y ±5 %
  (×4 manteniendo más de 1 s), como pide la tabla; A solo no hace nada. Desvío: **A + B** vuelve a 120 BPM o al swing
  recto (50 %). El swing muestra el nombre de los valores de referencia (recto, tresillo en 66–67 %, medio paso). Select
  + L2/R2 sigue cambiando el tempo en todas las pantallas. Se guardan en el proyecto (no en `config.ron`).
- Octava, CPU y la carpeta de datos se muestran como información. Los ajustes (fuente, audición, seguir reproducción)
  se guardan en `config.ron`.
- Las acciones se ejecutan con **un toque de A**; un doble toque no las repite. La prueba de controles y la muestra
  de fuentes se dibujan solo si entran (con la fuente de 12×24 se omiten).
- Select + Start mantenido 1 s y SALIR pasan por el mismo diálogo. En el simulador, Esc pregunta y un segundo Esc sale.

### 8. Navegador de archivos (modal)

```
CARGAR SAMPLE                      /groovebox/samples/baterias/
--------------------------------------------------------------------------------
    ..
    808/
    breaks/
  > kick_01.wav                     0,45 s   mono     16 bit   44,1 kHz
    kick_02.wav                     0,61 s   mono     16 bit   48 kHz
    snare_rim.wav                   0,20 s   estéreo  24 bit   48 kHz
    hat_open.wav                    1,10 s   mono     16 bit   44,1 kHz




--------------------------------------------------------------------------------
 A: abrir / elegir   X: escuchar   izq: carpeta de arriba   B: cancelar
 B+arr/abj: página
```

| Control | Solo | + B |
|---|---|---|
| ↑ / ↓ | Mover | Página |
| ← | Carpeta de arriba | — |
| → o A | Entrar en la carpeta / elegir el archivo | — |
| X | Escuchar el sample (a su tono original, por la pista actual) | — |
| B | Cancelar y volver | — |

El mismo navegador sirve para "Cargar proyecto" (lista carpetas de proyectos) con el título cambiado.

**Implementado (H6) para proyectos:** título "CARGAR PROYECTO" y la carpeta a la derecha; una fila por carpeta de
`proyectos/` que tenga `proyecto.ron` o `.bak`, por nombre, con la fecha del último guardado (en UTC: la consola no
tiene zona horaria configurada) y las marcas `(abierto)` y `solo .bak`. ↑/↓ mueven, B + ↑/↓ saltan 10, A o → abren
(con cambios sin guardar, primero el diálogo), B cancela. No hay subcarpetas ni X (eso llega con los samples, H7).

### 9. Editor de nombres (modal)

```
NOMBRE DEL PROYECTO
--------------------------------------------------------------------------------

      b  e  a  t  _  0  1 [ ]
                          ^
      a b c d e f g h i j k l m n o p q r s t u v w x y z 0 1 2 3 4 5 6 7 8 9 _ -

--------------------------------------------------------------------------------
 izq/der: mover   A+arr/abj: cambiar letra   A+B: borrar   Start: aceptar
 B: cancelar
```

Caracteres permitidos: `a–z`, `0–9`, `_` y `-`, hasta 32 (por compatibilidad con FAT; ver [spec-formatos](spec-formatos.md#nombres-de-archivo)).

**Implementado (H6)** como el mockup: el cursor puede estar en el hueco `[ ]` después de la última letra, y ahí A + ↑
agrega una `a` (A + ↓, un `-`). A + ←/→ también mueven el cursor (para no soltar A), ↑/↓ sin A cambian la letra igual
que con A, B + ←/→ saltan al principio o al final, A + B borra la letra bajo el cursor (en el hueco, la anterior).
Start acepta (un nombre vacío no se acepta) y B cancela. Abajo dice dónde va a quedar (`proyectos/<nombre>/`).

### 10. Diálogo de confirmación (modal)

```
              +--------------------------------------------------+
              |  Hay cambios sin guardar en "beat_01".           |
              |  ¿Salir igual?                                   |
              |                                                  |
              |      A: salir sin guardar      B: cancelar       |
              +--------------------------------------------------+
```

**Implementado (H6)** con una opción más: **Start: guardar y salir** (o "y crear el nuevo", "y abrir"); si el
guardado falla, no se sigue y se ve el error. Las opciones van una por línea para que entren con la fuente de 12×24.
El mismo recuadro se usa para "¿abrir la copia anterior (`.bak`)?" cuando `proyecto.ron` no se puede leer y para
"¿reemplazar el proyecto que ya existe?" en GUARDAR COMO. Solo un toque de A confirma: el segundo toque de un doble
toque (el A que abrió el diálogo y otro en menos de 300 ms) se ignora, así no se descartan cambios por accidente.

## Modo selección

| Control | Acción |
|---|---|
| Cruz | Extender la selección desde el ancla |
| Y | Ampliar: celda → columna, pista o fila → todo (frase, sección, cadena o bloque de la canción) |
| A | Copiar y salir |
| X | Cortar (copiar y borrar) y salir |
| B | Cancelar |
| A + cruz **(v1)** | Transponer la selección (±1 / ±12) o escalar velocidades |

**Pegar** es Select + A, fuera del modo selección, en la posición del cursor. El portapapeles es único y recuerda su tipo
(pasos, entradas de cadena, celdas de canción, instrumento); pegar un tipo incompatible muestra un aviso y no hace nada.

**Copiar frente a clonar:** copiar y pegar números de frase o de cadena **comparte** el mismo dato (editar uno cambia todos, y se marca con `*`).
Para tener una copia independiente se usa **A + Y** (clonar).

**Implementado (H10)** en Grilla, Frase, Canción, Cadena e Instrumento (Proyecto y Mezcla no tienen selección). Lo
seleccionado se dibuja con fondo azul y el pie muestra **SELECCIÓN**, el tamaño ("3 pistas × 8 pasos") y los controles;
copiar, cortar y pegar dejan un mensaje con cuántos elementos. En la Grilla y la Frase se copian **valores** de pasos;
en la Canción y la Cadena, **referencias** (cortar quita las referencias y no borra cadenas ni frases); en Instrumento,
los parámetros. Pegar pasos crea la frase que falte (creación implícita) y descarta lo que no entra; pegar entradas de
cadena en una fila sin cadena la crea. Mientras la selección está abierta, la vista no sigue la reproducción.
Desvíos (FRA no seleccionable, posiciones enteras en la Cadena, "bloque" = filas no vacías contiguas, la Frase pega en
las columnas copiadas, X en Instrumento no corta, sin confirmación al pegar un instrumento de otro tipo) y la tabla
completa de qué copia, corta y pega cada pantalla: [README del groovebox](../../../projects/groovebox/README.md#modo-selección-h10).

## Tipografía y legibilidad

- 640×480 en 3,5" son unos **229 ppp**: una fuente de 8×16 px mide ~1,8 mm de alto. Es chica para una sesión larga.
- Propuesta: fuente bitmap de **8×16** para las vistas densas (Frase, Canción, Proyecto) y bloques gráficos grandes para la Grilla
  (que no dependen de la fuente). Si la prueba en la consola (hito H1) muestra que no alcanza, pasar a **12×24** (53×20 caracteres)
  y reducir columnas: en la Frase, mostrar LOCK/COND/MICRO/REP en una segunda "página" de columnas; en Canción, mostrar 4 pistas por página.
- Fuentes candidatas con licencia libre: Terminus (OFL) o Spleen (BSD) **(sin verificar las licencias exactas)**. Se incluye la fuente en el binario.
- Colores: fondo oscuro, alto contraste. Un color por tipo de pista (percusión, sample, sinte). El cursor se dibuja invertido. El valor en edición se resalta en otro color.
- Refresco: redibujar solo cuando cambia algo o hay reproducción (cabezales), con un tope de 60 fps.

## Preguntas abiertas

1. ¿El esquema **A = editar, B = cancelar/borrar con A, Select = global** te resulta cómodo, teniendo en cuenta la posición real de los botones en la RGB20S (la cruz está debajo del stick izquierdo)? Conviene probarlo en H1.
2. Borrar con **A + B** (dos botones) es más seguro pero más lento que "tocar A sobre un paso con nota lo borra" (estilo caja de ritmos). ¿Cuál preferís en la Grilla?
3. ¿Sticks sobre la **pista actual** (propuesto) o sobre el **master** (más "DJ")?
4. ¿Tamaño de fuente: 8×16 o 12×24? Se decide con la prueba en la consola.
5. ¿Valores decimales 0–127 (propuesto) o 0–99 (más fáciles de leer, menos compatibles con MIDI)?
6. **(nueva)** ¿Start **contextual** según la pantalla (propuesto: canción en Canción/Grilla, cadena en Cadena, frase en Frase) o siempre la canción completa?

Respondidas el 2026-11-19: ~~acordes~~ (columna ACORDE con tipos predefinidos e inversión), ~~creación implícita desde la Grilla~~ (sí).

## Fuentes

- [Manual de operación M8 v3.2.0](https://cdn.shopify.com/s/files/1/0455/0485/6229/files/m8_operation_manual_v20230630.pdf) (convenciones de teclas, mapa de vistas, selección, clonado, filas aisladas en la canción).
- [learnlsdj](https://learnlsdj.github.io/index.html) (navegación Select + cruz, canción → cadena → frase).
- [Thomann: reviews de usuarios del Deluge](https://www.thomannmusic.com/synthstrom_audible_deluge_reviews.htm) (información oculta en pantallas chicas).
- [Retro Dodo: RGB20S](https://retrododo.com/powkiddy-rgb20s/) y [retrocatalog: RGB20S](https://retrocatalog.com/retro-handhelds/powkiddy-rgb20s) (botón FN, L3/R3, volumen, posición de la cruz).
- [Referentes](referentes.md).
