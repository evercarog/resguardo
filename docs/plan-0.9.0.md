# Plan de la 0.8.1 y la 0.9.0 (propuesta para revisar)

Propuesta del 2026-10-19, **pendiente del visto bueno del responsable**. Recoge el bloque 15 de [plan-mejoras.md](plan-mejoras.md) (15a–15rr, apuntado durante la primera semana de uso real de la 0.8.0) y lo que quedó abierto de los bloques 11–14. El detalle de cada punto está en `plan-mejoras.md`; aquí, el orden, el alcance y las decisiones. Marca aquí el estado de cada bloque (`[ ]` pendiente, `[~]` en curso, `[x]` hecho, con la rama). Lo que cambie mensajes entre agente y consola va en `docs/api-servidor.md` → «Cambios», compatible hacia atrás.

**Objetivo de la 0.9.0: pulir.** Menos pantallas y más claras, ver de un vistazo dónde está cada cosa, que nada falle en silencio y que lo de cada día (renombrar, excluir, restaurar, desplegar) sea fácil. Pocas funciones nuevas grandes.

**Principio (2026-10-16):** Resguardo es para cualquiera, no para un cliente concreto. Cada punto se diseña para cualquier entorno (equipo físico, Hyper-V, VMware, Proxmox, NAS, nube); lo propio de un proveedor, como extra opcional.

## 0.8.1 — Arreglos que estorban el despliegue (antes, corta)

Una versión pequeña, solo fallos, para desplegar ya muchos equipos sin rodeos. Con su prueba cada uno.

- [ ] **R1. Instalación silenciosa que no vincula (15ff, causa real de 14ee):** el instalador corta `/SERVIDOR=https://…` en la primera `/`. Leerlo bien, la línea de la consola con comillas, y **prueba real en el CI**: compilar el instalador, instalarlo en silencio en el Windows del CI y comprobar que vincula.
- [ ] **R2. Quitar la derivada «de cada día» no la quita de la consola (15p):** limpiar `externa` al quitarla y, al arrancar, las que quedaron así.
- [ ] **R3. El instalador de «Instalar muchos equipos» se queda viejo (15x):** cambiarlo solo al bajar una versión más nueva; la página dice qué versión reparte; «Cambiarlo» también si se puso a mano.
- [ ] **R4. «Ordenar nombres de carpeta» rompe las derivadas hacia el almacén (15i).**
- [ ] **R5. Copia cortada al apagar sin rastro ni reintento (15jj):** anotarla «en curso» al empezar, «interrumpida» al arrancar, y reintentarla.

## 0.9.0 — Pulir

### Bloque A — Que nada falle en silencio

- [ ] **A1.** Un repositorio que sale de un espejo avisa, y lo que queda en el destino sin actualizar se ve (15m).
- [ ] **A2.** La página del repositorio conoce sus derivadas, y cada derivada tiene su página con «Retención en el almacén…» (15g).
- [ ] **A3.** Volcados de SQL: el permiso que falta, dicho claro y como aviso, no como fallo (15f); «todas las bases» y aviso de bases nuevas (15u).
- [ ] **A4.** Los errores de las copias en palabras, con qué hacer, y el detalle técnico plegado (15oo); el historial corta las líneas largas (15j).
- [ ] **A5.** Parar y pausar una copia en marcha, con la clave de administración; restic muere con el servicio (15hh).
- [ ] **A6.** Que el equipo no se suspenda mientras copia (15ll).
- [ ] **A7.** Quitar un repositorio vacío solo con la clave; `kit` y `copias` ven todos los repositorios del equipo (15q). Lo que queda de 14nn.

### Bloque B — Menos pantallas, más orden

- [ ] **B1. Menú izquierdo de ~8 entradas con pestañas** (15bb): Estado · Equipos · Copias · Destinos · Recuperar · Avisos · Actividad · Informes, y Ajustes aparte. Probarlo con alguien que no conozca la consola.
- [ ] **B2. «Organización»** (hoy «Sedes y categorías»): sedes, categorías, **etiquetas** (renombrar, borrar, unir) y **grupos de exclusiones** (15a, 15y).
- [ ] **B3. Un almacén, una sola ficha** (15dd) y su tarjeta por secciones: espejos, retención y nubes, con página propia de cada nube (15s); «Ordenar nombres de carpeta» como aviso suave (15h).
- [ ] **B4. Clientes:** borrar o archivar uno, aviso de nombre repetido y un camino claro para traer equipos de otra consola (15nn).
- [ ] **B5. Lo que describe los datos, igual en todas las consolas:** nombres y observaciones de equipos, copias, repositorios y destinos (15pp).
- [ ] **B6.** Las derivadas con su nombre en todas las listas (15k).

### Bloque C — Ver dónde está cada cosa

- [ ] **C1. «Dónde está»** en cada repositorio y copia, con la ruta real en cada sitio, su tamaño y si está al día (15n).
- [ ] **C2. El mapa:** grupos que se despliegan y agrupación por sede (15rr), derivadas y espejos animados (15cc), vista «Mapa» en Equipos (15b) y capas (15l, empezando por Sedes, Copias y Estado).
- [ ] **C3. Tipo de equipo** (físico, VM, contenedor), detectado y corregible (15c).
- [ ] **C4. Infraestructura, fase 1, para cualquier entorno** (15d): lo que el agente detecta solo, el resto anotado con ayuda, una línea por destino. Sin conectar ningún hipervisor (15e aparcada).

### Bloque D — Las copias de cada día, más fáciles

- [ ] **D1. Renombrar** una copia, un espejo o una derivada con un lápiz y la clave; el nombre arriba del asistente (15t).
- [ ] **D2. Grupos de exclusiones** de un toque, gestionables por cliente, «no copiar más de N MB» y vista previa de lo que se deja fuera (15y).
- [ ] **D3. Retención «Personalizada»** desde cero o desde la elegida (15z).
- [ ] **D4. Etiquetas de las versiones** por copia y filtro en el historial (15v).
- [ ] **D5. Espejo desde una copia** con su repositorio marcado, o añadirlo a uno que ya existe (15r).
- [ ] **D6. Traer repositorios viejos al almacén** sin terminal: repositorios sueltos del disco del almacén, «Dar a un equipo» y adoptar (15gg).
- [ ] **D7. Equipos que entran y salen:** «además, al volver a conectarse», avisos de «lleva N días fuera» y no empezar sin batería (15ii).
- [ ] **D8. Apagar el equipo al terminar** una copia, con la clave y aviso al usuario (15kk).

### Bloque E — Restaurar

- [ ] **E1.** El árbol de archivos marca lo de dentro y deja quitar lo que no (15w).
- [ ] **E2.** El motivo **antes** de restaurar, obligatorio por defecto, común a todas las consolas (15ee).
- [ ] **E3.** Las pruebas de restauración automáticas y a mano en «Restauraciones», con su ficha (15aa).
- [ ] **E4.** Lo que quedó de la 0.8.0: restaurar desde los espejos del propio equipo (14m) y la prueba desde un disco del almacén (14j).

### Bloque F — El agente en Windows

- [ ] **F1.** `resguardo-agente` en el PATH, la bandeja tras instalar, y que diga por qué no abre la ventana (elevada o sin WebView2) (15o).
- [ ] **F2.** Instalar sin rodeos con ESET, Intune o GPO: guía con las órdenes en forma `cmd` (15x) y, con R1, una sola tarea.

## Para decidir antes de empezar

1. **¿Una 0.8.1 primero** (R1–R5, en unos días) y luego la 0.9.0? Recomendado: sí, desbloquea el despliegue por ESET.
2. **El nombre:** «0.9.0» para la versión de pulir.
3. **¿Entran ya o se dejan para después?** Son grandes o nuevas, y la 0.9.0 es para pulir:
   - 15mm, que la persona del equipo recupere sus archivos desde el menú contextual;
   - 15qq, la evidencia para cumplimiento (paquete para auditoría);
   - D7 (portátiles) y D8 (apagar al terminar);
   - C4, la infraestructura fase 1.
   Recomendación: C4 y D7 dentro; 15mm, 15qq y D8 para la siguiente.
4. **Seguridad de la consola (bloque 12):** passkeys (12c) y regla de dos personas (12d) siguen pendientes. Recomendación: 12c en la 0.9.0 si sobra tiempo; el resto, en su propia versión.
5. **14v** (derivadas filtradas hechas por el almacén) sigue en «no».

## Fuera de este plan

- 11f: probar la actualización real en máquinas virtuales. 11g: que Resguardo Server se actualice solo.
- 15e: conectar Proxmox u otro hipervisor (aparcada, opcional).
- Mac, instaladores con firma comercial y volcados de otras bases de datos (decisión anterior).
