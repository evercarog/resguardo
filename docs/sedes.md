# Sedes, categorías de etiqueta y orden de los equipos (plan 0.8.0, D1 y D2)

Escrito el 2026-10-14 con el responsable ausente (mandato de la 0.8.0). Puntos 14ll y 14ss de [plan-mejoras.md](plan-mejoras.md). Contrato en [api-servidor.md](api-servidor.md) «Cambios»; la regla, en [regla-3-2-1.md](regla-3-2-1.md).

## Qué es una sede

Un **dato propio del cliente** (no una etiqueta; decisión del responsable, 2026-10-09): una lista de sitios con nombre y notas («Oficina Norte», «Planta Sur»), una marcada como **principal**. Cada **equipo** y cada **destino** está en una sola sede. La **nube no es sede**: siempre está fuera del sitio.

- **Sin asignar**, un equipo está en la principal (o, si ninguna lo es, en la primera por nombre); la consola avisa en Equipos de los que no tienen sede asignada.
- **Sin asignar**, un destino que vive en un equipo (la zona de un almacén, un disco o una carpeta, el espejo de un almacén en otro disco) está en la sede de ese equipo; si una persona marcó su tipo a mano (por ejemplo, un disco que se lleva a casa marcado «Fuera del sitio»), se respeta su tipo. Un servidor de fuera o un NAS sin sede asignada no tiene sede: manda su tipo, como antes.
- **Sin sedes en el cliente**, todo funciona como antes: el tipo del destino (Local, Fuera del sitio o Nube) dice si está fuera.

## En la regla 3-2-1-1-0

«Fuera del sitio» = el destino está en **otra sede que el equipo que copia**. El mismo almacén está en el sitio para los equipos de su sede y fuera para los demás. Lo mismo para el aviso «Lo inmutable o aislado está todo en el sitio» (usa la misma pregunta). La regla está en `protection.rs` y en `regla321.ts` con los vectores compartidos (`OrigenRegla.sede`, `PasoRegla.sede`); el agente no conoce las sedes, así que la consola las pone al armar la entrada (`reglaDeCopia`), como ya hacía con los espejos del almacén y el catálogo.

El aviso «Copias en el mismo equipo» (`riesgoMismoEquipo`) cuenta ahora también las copias derivadas activas a otro sitio (antes solo la copia externa).

## Cómo se guardan y viajan

Como los demás datos comunes del cliente (colores de las etiquetas, catálogo de destinos; [consolas-multiples.md](consolas-multiples.md) §6.5): `sede:`, `sede.equipo:`, `sede.destino:`, `etiqueta.categoria:` y `orden.equipos:todos`. Se guardan en el servidor al momento y se reparten a los equipos (solo a los que anuncian `admite: "sedes"`), que los suben a todas sus consolas. **La sede de un equipo o de un destino pide la clave de administración** para repartirla (cuenta en la regla, como el tipo y las marcas de un destino): se cambia aquí al momento y el aviso «Datos comunes» ofrece repartirla con la clave. La lista de sedes, las categorías y el orden no la piden.

Las ve cualquier miembro del cliente (`GET /api/clientes/{c}/organizacion`); las cambian administradores y propietarios.

## Vista de Equipos (14ss)

- **Categorías de etiqueta** («Tipo»: Servidor, Puesto): un solo valor por equipo; al poner una etiqueta de una categoría se quita la otra de la misma. Una etiqueta va en una sola categoría. Las sueltas siguen igual.
- **Agrupar por** dos niveles (sede, cada categoría, sistema exacto si los equipos mandan sus datos básicos, etiquetas sueltas), **sin duplicados**: un equipo con varias etiquetas sueltas va en el grupo de todas juntas («Contabilidad · Ventas»). Grupos plegables con cuántos equipos y cuántos necesitan atención; «Sin …» al final.
- **Orden**: «Lo que necesita atención primero», «Por nombre» o «A mano». A mano, flechas (teclado) o arrastrar **dentro de cada grupo**; el orden se guarda en el servidor y es igual en todas las consolas.
- La agrupación, el orden elegido y los grupos plegados se recuerdan en el navegador, por persona (su cuenta) y cliente.

## Decisiones dudosas

- Sede del destino sin asignar = la de su equipo; un servidor de fuera sin asignar no tiene sede (manda su tipo). Alternativa: la principal también para los destinos.
- La sede de equipos y destinos pide la clave para repartirse (coherente con el tipo de un destino); asignarla en esta consola es inmediato.
- «Recordar por persona» se hace en el navegador con la cuenta en la clave (no en el servidor).
