# Capacidad: cuánto pide la consola y cuánto aguanta el servidor

Qué carga ponen las consolas abiertas y los equipos sobre Resguardo Server, los límites que la protegen y lo medido (octubre de 2026, contrato v1.42).

## Lo que pide una consola abierta

Medido con un navegador contra un servidor de verdad con dos agentes (`consola/scripts/e2e/consola-abierta.ts`), en reposo, con el canal en vivo abierto, peticiones por minuto de **una pestaña**:

| Pantalla | Antes (0.7.18) | Ahora |
|---|---|---|
| Añadir equipo | **≈ 3400** (`GET …/emparejamientos` en bucle) | 16 (la lista de preparados cada 5 s, el progreso, el resumen) |
| Estado del cliente | 5 | 5 |
| Equipos | 5 | 5 |
| Un equipo | 4 | 4 |
| Todos los clientes | 6 | 6 |

Con algo en marcha (copias, órdenes) la consola pide más mientras dura: el progreso cada 3 s sin canal en vivo (cada 30 s con él, más una pregunta «ya» como mucho cada 2 s cuando el canal avisa), las órdenes pendientes cada 2 s. Presupuesto: **≤ 30 peticiones por minuto en reposo** y **≤ 120 con algo en marcha** por pestaña.

Lo que lo mantiene así (y lo comprueban `npm run test:vectores` en `consola/` y las pruebas del servidor):

- Ningún `$effect` lanza una carga siguiendo lo que lee (el fallo de «Añadir equipo»: el efecto leía la lista que él mismo cambiaba y volvía a pedir sin pausa). Una prueba recorre los 86 efectos de la consola.
- Las cargas por avisos del canal van con freno (`lib/freno.ts`): como mucho una cada 2 s por pantalla, nunca dos a la vez, lo de en medio en una al final.
- El servidor junta los avisos repetidos: el mismo aviso a un cliente, como mucho una vez cada 2 s (`vivo.rs`, `JUNTAR`), con uno al final. Un equipo que se conecta y se cae en bucle no multiplica las peticiones de las consolas; y el agente reabre su canal con esperas crecientes (hasta 1 min) si se cierra una y otra vez.
- Con la pestaña oculta no se pregunta (salvo el canal en vivo, que no pide nada).
- Tras un 429 de los límites generales, la consola no hace más peticiones de lectura hasta `retry_after` (`lib/pausa429.ts`).
- Una pestaña abierta antes de actualizar el servidor sigue con el código de antes: la consola lo detecta y ofrece «Recargar».

## Lo que pide un equipo

Con el canal (WebSocket) abierto, casi nada por HTTP: el informe cuando cambia algo (y como mucho cada 5 min sin cambios), el progreso cada 10 s mientras hay algo en marcha, y los resultados de las órdenes. Sin canal, una consulta por minuto (cada 2 s durante 10 min si la consola pide atención). Del orden de **1–3 peticiones por minuto** por equipo en reposo.

## Límites (y por qué una cuenta no puede tumbar a las demás)

| Límite | Valor | Respuesta |
|---|---|---|
| Peticiones por cuenta con sesión | 1200 / min | 429 `limite: "cuenta"` con `retry_after` |
| Peticiones a la API por IP (salvo este mismo equipo) | 1800 / min | 429 `limite: "ip"` con `retry_after` |
| Códigos nuevos para añadir equipos | 30 / h por cuenta y cliente; 100 / h por cliente | 429 `limite: "codigos"` con `retry_after` |
| Intentos de contraseña, TOTP… | los de cada ruta | 429 `limite: "intentos"` |
| Conexiones del canal en vivo | 12 por cuenta, 100 por cliente, 5000 en total | 429 al abrir |

Los límites son **por cuenta y por IP**: si una consola se desboca (como la de «Añadir equipo» antes de 0.7.19), solo se bloquea esa cuenta, un minuto; las demás personas siguen. Cada límite que salta queda en el registro, como mucho una vez por minuto, IP y límite («Límite de intentos superado desde IP: cuenta (GET …)»), también en `servidor.log` de la carpeta de datos (en Windows, el servicio no tiene otra salida).

## Cuánto aguanta el servidor

`consola/scripts/e2e/carga.ts`: un servidor de verdad (compilación *release*), 50 clientes, dos agentes reales y 150 «pestañas» de 10 cuentas pidiendo lo que pide la consola en reposo (el peor caso medido, 16 por minuto), y después cuatro veces eso:

| Carga | Peticiones / min | Latencia p50 / p95 / p99 | Errores | CPU del servidor | Memoria |
|---|---|---|---|---|---|
| 150 pestañas en reposo (×1) | 2 243 (37/s) | 2 / 5 / 7 ms | 0 (todas 200) | 4 % de un núcleo | 30 MB |
| ×4 | 8 908 (148/s) | 28 / 42 / 94 ms | 0 (todas 200) | 12 % de un núcleo | 41 MB |

(Windows 11, el mismo equipo para el servidor y quien pide; las latencias de ×4 incluyen la espera de quien pide, que es un solo proceso de Node.)

Para «50 clientes × 10 equipos × 3 consolas abiertas»: 150 pestañas en reposo son ≈ 2400 peticiones por minuto (40 por segundo) y 500 equipos ≈ 500–1500 por minuto más: ≈ 3 000–4 000 por minuto, entre la carga ×1 y la ×4 medidas, unos pocos por ciento de un núcleo y menos de 50 MB. Lo que de verdad lo pone en peligro no es el número de consolas sino una consola en bucle: una sola pestaña de «Añadir equipo» de 0.7.18 pedía tanto como 200 pestañas normales (y por eso saltaba su límite por cuenta en segundos).

Para repetir la medida: `cargo build --release -p resguardo-servidor --bin resguardo-server`, `cargo build -p resguardo-agente --bins` y, en `consola/`, `npx tsx scripts/e2e/carga.ts` (variables `PESTANAS`, `POR_MINUTO`, `FASES`, `SEGUNDOS`, `CLIENTES`).
