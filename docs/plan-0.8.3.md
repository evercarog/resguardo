# Plan de la 0.8.3 (pulido y estabilidad)

Propuesto el 2026-10-22, tras la 0.8.2 y la decisión de «probar y pulir». **Pendiente del visto bueno del responsable.** Sin funciones grandes nuevas: lo que sale del uso real, de las pruebas en máquinas virtuales ([pruebas-reales.md](pruebas-reales.md)) y de una pasada de sencillez. Detalle de cada punto en [plan-mejoras.md](plan-mejoras.md), bloque 16. Rama de trabajo: `ia/083-pulido` (se une a `main` **aplastada**: tiene un commit con un nombre real, ya corregido).

## A. Fallos y lo que engaña (primero)

- [ ] **16j.** El mapa con los espejos y las derivadas que sí funcionan (un nodo por destino, una línea por repositorio, contar bien los espejos de una zona, una nube sin «ubicación física»).
- [~] **16l.** La línea verde de carga cada pocos segundos (hecho lo principal; revisar las cargas en serie).
- [ ] Lo que salga de las pruebas en máquinas virtuales.

## B. Que se entienda sin explicarla

- [ ] **16m.** Inicio del cliente que conteste «¿estoy protegido?».
- [ ] **16n.** Menos vocabulario a la vista (con alguien que no conozca Resguardo).
- [ ] **16o.** Cada aviso con un solo «qué hago», agrupados, resumen diario opcional.
- [ ] **16p.** Ningún ajuste lejos de donde se nota; ningún control que no haga nada.
- [ ] **16c.** «Actualizaciones de los agentes» en «Mi cuenta y servidor».
- [ ] **16g.** Los equipos de usuario no salen «Sin anotar» en Infraestructura.
- [ ] **16s.** Primeras veces y estados vacíos.

## C. Comodidad

- [ ] **16k.** Sesión renovada con el uso, «Recordar este dispositivo», lista de sesiones y una sesión nueva sin poder sobre las demás durante 24 h.
- [ ] **16r.** Lo que se hace desde el móvil, a dos toques.
- [ ] **16e.** Más filtros en Equipos y en el mapa (sede, varias etiquetas, estado…), con vistas guardadas.
- [ ] **16t.** Tablas largas cómodas.
- [ ] **16u.** Errores siempre en palabras.
- [ ] **16b.** Ver qué archivos no se pudieron leer, y «No copiar esto».
- [ ] **16h.** «Este equipo no llega a esa consola» para los que no tienen internet a propósito.

## D. Infraestructura (pequeño)

- [ ] **16a.** Marca de cada disco (hecho: «HDD» sin «(de platos)» y el número de serie en «Hecho de:»).
- [ ] **16f.** Número de serie del servidor físico.
- [ ] **16d.** Página de cada disco y de cada conjunto con lo que hay dentro.

## E. Rendimiento y confianza

- [ ] **16q.** Rendimiento con ~200 equipos.
- [ ] **16v.** «Comprobar todo» de un cliente.

## Fuera de esta versión (para hablarlo)

- **16i.** Copia del estado del sistema de Windows (controladores de dominio): es una función nueva; mientras tanto, la receta a mano con `wbadmin`.
- **11g.** Que Resguardo Server se actualice solo.
- **12a/12e.** Consola verificada y revisión de seguridad externa.

## A decidir con el responsable

- ¿Entra todo B y C, o se recorta para que la versión sea corta?
- ¿Con quién se prueba la consola «sin conocerla» (16n) antes de cerrar la versión?
