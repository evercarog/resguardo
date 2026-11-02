# Plan de la 0.8.3 (pulido y estabilidad)

Propuesto el 2026-10-22, tras la 0.8.2 y la decisión de «probar y pulir». **Visto bueno del responsable el 2026-11-02** («trabaja en la nueva versión»): entra todo; lo de los antivirus (16z) va lo primero porque ya paró copias en dos clientes. Sin funciones grandes nuevas: lo que sale del uso real, de las pruebas en máquinas virtuales ([pruebas-reales.md](pruebas-reales.md)) y de una pasada de sencillez. Detalle de cada punto en [plan-mejoras.md](plan-mejoras.md), bloque 16. Rama de trabajo: `ia/083-pulido` (se une a `main` **aplastada**: tiene un commit con un nombre real, ya corregido).

## A. Fallos y lo que engaña (primero)

- [ ] **16z. Antivirus que pone el agente en cuarentena** (dos clientes el mismo día, 2026-10-25, la misma detección heurística contra la 0.8.2). En esta versión: (a) el ejecutable con datos de versión, fabricante, descripción, icono y manifiesto (hoy no lleva ninguno, y eso sube la sospecha de las heurísticas); (b) el instalador comprueba al terminar que el programa sigue ahí y que el servicio arranca, y si no, lo dice con el motivo probable; (c) la consola, cuando un equipo deja de responder justo después de una copia correcta, sugiere mirar el antivirus; (d) ayuda «El antivirus lo bloqueó» y paso en la receta de publicación: enviar cada versión al análisis de falsos positivos de Microsoft antes de soltarla. La firma de código queda para decidir (de pago).
- [x] **16y (resto).** Huecos perdidos en el mapa de calor y `copiaAtrasada` en la salud.

- [x] **16j.** El mapa con los espejos y las derivadas que sí funcionan (un nodo por destino, una línea por repositorio, contar bien los espejos de una zona, una nube sin «ubicación física»).
- [x] **16l.** La línea verde de carga cada pocos segundos (también las cargas en serie: solo cuenta lo que sigue a una acción de la persona).
- [ ] Lo que salga de las pruebas en máquinas virtuales.

## B. Que se entienda sin explicarla

- [x] **16m.** Inicio del cliente que conteste «¿estoy protegido?».
- [ ] **16n.** Menos vocabulario a la vista (con alguien que no conozca Resguardo).
- [~] **16o.** Cada aviso con un solo «qué hago», agrupados, resumen diario opcional.
- [ ] **16p.** Ningún ajuste lejos de donde se nota; ningún control que no haga nada.
- [x] **16c.** «Actualizaciones de los agentes» en «Mi cuenta y servidor».
- [ ] **16g.** Los equipos de usuario no salen «Sin anotar» en Infraestructura.
- [ ] **16s.** Primeras veces y estados vacíos.

## C. Comodidad

- [ ] **16k.** Sesión renovada con el uso, «Recordar este dispositivo», lista de sesiones y una sesión nueva sin poder sobre las demás durante 24 h.
- [ ] **16r.** Lo que se hace desde el móvil, a dos toques.
- [ ] **16e.** Más filtros en Equipos y en el mapa (sede, varias etiquetas, estado…), con vistas guardadas.
- [ ] **16t.** Tablas largas cómodas.
- [x] **16u.** Errores siempre en palabras.
- [ ] **16b.** Ver qué archivos no se pudieron leer, y «No copiar esto».
- [ ] **16h.** «Este equipo no llega a esa consola» para los que no tienen internet a propósito.

## D. Infraestructura (pequeño)

- [ ] **16a.** Marca de cada disco (hecho: «HDD» sin «(de platos)» y el número de serie en «Hecho de:»).
- [ ] **16f.** Número de serie del servidor físico.
- [ ] **16d.** Página de cada disco y de cada conjunto con lo que hay dentro.

## E. Rendimiento y confianza

- [ ] **16q.** Rendimiento con ~200 equipos.
- [ ] **16v.** «Comprobar todo» de un cliente.
- [x] **16w.** Lo que ocupa de verdad en disco, también en los totales.
- [ ] **16x.** Rendimiento y consumo: medir antes de tocar; ajustes suaves y en orden (primero, la consola comprimida).

## Fuera de esta versión (para hablarlo)

- **16i.** Copia del estado del sistema de Windows (controladores de dominio): es una función nueva; mientras tanto, la receta a mano con `wbadmin`.
- **11g.** Que Resguardo Server se actualice solo.
- **12a/12e.** Consola verificada y revisión de seguridad externa.

## A decidir con el responsable

- ~~¿Entra todo B y C?~~ Sí (2026-11-02).
- ¿Con quién se prueba la consola «sin conocerla» (16n) antes de cerrar la versión?
- Firma de código de los ejecutables (16z, punto 5).
