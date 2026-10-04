# crates/agente

El agente de Resguardo y lo que comparte con la app de escritorio, **sin Tauri**:

- copias programadas y tareas (`agent`, `tasks`, `history`, `protection`, `restore_test`);
- configuración y secretos (`store`, `places`, `kit`);
- Servidor de copias (`server`);
- equipos gestionados de la fase 5 (`endpoint`, `console`, `managed`, `agente`) y la web de Supabase (`web`, `share`, `remote`);
- el **agente v2** de Resguardo Server (`servidor_v2`) y su línea de órdenes en español (`cli`).

El binario es `resguardo-agente` (`src/bin/resguardo-agente.rs`). La app de escritorio (`src-tauri`) depende de este crate y reexporta sus módulos.
