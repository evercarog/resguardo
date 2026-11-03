# Pruebas en equipos reales (fase de estabilización tras la 0.8.2)

Decidido el 2026-10-21: unos días sin funciones nuevas, centrados en **probar y pulir**. Lo que salga se apunta en [plan-mejoras.md](plan-mejoras.md) (bloque 16); los fallos se arreglan en la rama `ia/083-pulido`.

Cómo usar esta lista: cada prueba con su resultado (`[x]` bien, `[!]` falló —con lo que pasó—, `[-]` no aplica) y la fecha. Lo ideal es un **laboratorio de máquinas virtuales** (por ejemplo en un Proxmox o Hyper-V que no sea de un cliente): un Windows Server, un Windows 10/11, un Debian o Ubuntu, una consola de pruebas y un almacén de pruebas. Nada de esto se hace primero en equipos de clientes.

## 1. Instalación y vinculación (lo más importante antes de desplegar)

- [ ] 1a. Instalar el agente **por comandos** en un Windows limpio, en silencio (`/S /CODE=… /SERVIDOR="https://…"`): instala **y** vincula; `instalador.log` dice el resultado. Probarlo en `cmd` y en PowerShell, como administrador y como SYSTEM (que es como lo lanzan las herramientas de despliegue: GPO, Intune, PDQ, un antivirus con consola…). No hace falta ninguna herramienta concreta: basta con que el comando funcione.
- [ ] 1b. Un Windows Server y un Windows 10/11, por si se comportan distinto.
- [ ] 1c. La línea de PowerShell de «Instalar muchos equipos» copiada tal cual.
- [ ] 1d. Un Windows Server 2016 sin WebView2: la bandeja y «Abrir Resguardo» dicen qué falta.
- [ ] 1e. Agente en Linux (Debian/Ubuntu) con `instalar-agente.sh`: instala, vincula y copia.
- [ ] 1f. Desinstalar en Windows: quita el servicio, el menú del Explorador y el `PATH`; los datos de copias se quedan.

## 2. Actualización automática y vuelta atrás (11f, pendiente desde la 0.7.25)

- [ ] 2a. Un agente 0.8.1 en el anillo «prueba» sube solo a la 0.8.2 (con «Buscar ahora»).
- [ ] 2b. Anillo «general»: espera sus días; «Actualizar ahora» lo adelanta.
- [ ] 2c. Con una copia en marcha, no se actualiza hasta que termina.
- [ ] 2d. Un almacén sin ventana de mantenimiento queda «Pendiente: necesita una ventana»; con ventana, se actualiza dentro de ella.
- [ ] 2e. **Vuelta atrás:** forzar que la versión nueva no arranque (p. ej. con una versión de prueba firmada con una llave de pruebas en un servidor de pruebas) y comprobar que vuelve sola y avisa.
- [ ] 2f. Linux: la misma actualización con `systemctl restart`.

## 3. Copias del día a día

- [ ] 3a. Una copia con un archivo abierto (Outlook, Excel): se copia con la instantánea de Windows.
- [ ] 3b. «Algunos archivos no se pudieron leer»: ver cuáles con el registro detallado (16b) y por qué.
- [ ] 3c. Parar y pausar una copia larga (con la clave); «En pausa hasta…» y sigue sola.
- [ ] 3d. Apagar el portátil a mitad de copia: al volver, «interrumpida» y se reintenta.
- [ ] 3e. Desconectar la red a mitad de copia: la consola dice «Interrumpida: … se desconectó».
- [ ] 3f. «Al terminar: apagar» en un equipo de pruebas: aviso de 2 min y se apaga.
- [ ] 3g. Un portátil «sale de la oficina»: sin avisos falsos y «Además, al volver a conectarse».
- [ ] 3h. Volcado de SQL Server con «todas las bases» y verificación (permiso `CREATE ANY DATABASE`).

## 4. Almacén, espejos y retención

- [ ] 4a. La retención del almacén poda y el espejo **no** frena (B3); borrar a mano un archivo del almacén **sí** frena.
- [ ] 4b. Un espejo a la nube (B2/Dropbox) y otro a un disco: suben, se verifican y se ven en el mapa.
- [ ] 4c. Quitar una copia derivada: desaparece de la tarjeta, el mapa y los avisos.

## 5. Restaurar

- [ ] 5a. Restaurar un archivo junto al original, en otra carpeta y en un .zip con contraseña (abrirlo con 7-Zip).
- [ ] 5b. El árbol: desmarcar una subcarpeta y comprobar que no se restaura.
- [ ] 5c. Restaurar desde un espejo (el almacén apagado).
- [ ] 5d. «Recuperar en el equipo» desde el Explorador con una cuenta de Windows **sin** privilegios: solo ve y recupera lo suyo.
- [ ] 5e. Una prueba de restauración automática y su ficha en «Restauraciones».

## 6. Varias consolas

- [ ] 6a. Un equipo en dos consolas (local y en línea): notas, pruebas funcionales, infraestructura y sedes iguales en las dos.
- [ ] 6b. Una orden con espera mandada en una y cancelada desde la otra.
- [ ] 6c. Dejar una consola desde la ventana del agente (con la clave): la otra sigue y la dejada lo dice.

## 7. Consola y seguridad

- [ ] 7a. Entrar con una llave de acceso (Windows Hello o el móvil) además de la contraseña.
- [ ] 7b. El informe de configuración y el paquete de auditoría en PDF y CSV: se abren bien en Excel y en un lector de PDF.
- [ ] 7c. La consola en el móvil (375 px): las páginas principales sin desbordes.
- [ ] 7d. Actualizar Resguardo Server en Linux con `instalar-servidor.sh` sin perder nada.

## Lo que se vio en el uso real (para no repetirlo)

Ver el bloque 15 y 16 de [plan-mejoras.md](plan-mejoras.md) y el resumen de cada versión en [registro-ia.md](registro-ia.md).
