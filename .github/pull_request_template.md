## Qué cambia y por qué

<!-- Una o dos frases. Enlaza el issue si lo hay: «Cierra #123». -->

## Cómo se ha probado

- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo test --workspace`
- [ ] `npm run check` (si toca la interfaz)
- [ ] Capturas en claro y oscuro (si cambia algo visible)

## Seguridad

- [ ] No añade secretos a argumentos, registros, la web ni el servidor.
- [ ] Si toca el protocolo o un analizador: vectores o pruebas de humo/fuzzing actualizados.

## Licencia

- [ ] He leído el [CLA](../CLA.md) (versión 1.0) y lo acepto para esta y mis próximas contribuciones.
- [ ] El código es mío, o indico aquí el origen y la licencia (compatible con AGPL-3.0) de lo que no lo es.
