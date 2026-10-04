// Pruebas del Markdown ligero de las observaciones y los comentarios
// (src/lib/markdown.ts): que nada de HTML escrito llegue a la página, que
// solo se enlacen http, https y mailto, y las marcas que sí admite.
//
//   npm run test:vectores
import { enlaceSeguro, escapar, markdown, primeraLinea } from "../src/lib/markdown";

let total = 0;
let fallos = 0;
function igual(nombre: string, real: unknown, esperado: unknown) {
  total++;
  const ok = JSON.stringify(real) === JSON.stringify(esperado);
  if (!ok) {
    fallos++;
    console.error(`✗ ${nombre}\n   real:     ${JSON.stringify(real)}\n   esperado: ${JSON.stringify(esperado)}`);
  } else console.log(`✓ ${nombre}`);
}
const cierto = (nombre: string, v: boolean) => igual(nombre, v, true);

console.log("\n— Markdown de las notas —");
igual("escapa", escapar(`<a href="x">'&'</a>`), "&lt;a href=&quot;x&quot;&gt;&#39;&amp;&#39;&lt;/a&gt;");
igual("párrafos y saltos", markdown("hola\nadiós\n\notro"), "<p>hola<br>adiós</p><p>otro</p>");
igual("negrita y cursiva", markdown("**Ojo**: *mucho* _cuidado_"), "<p><strong>Ojo</strong>: <em>mucho</em> <em>cuidado</em></p>");
igual("snake_case no es cursiva", markdown("base_de_datos y 2*3*4"), "<p>base_de_datos y 2*3*4</p>");
igual("código", markdown("ejecuta `<b>restic</b> check`"), "<p>ejecuta <code>&lt;b&gt;restic&lt;/b&gt; check</code></p>");
igual("listas", markdown("- uno\n* dos\n1. tres\n2) cuatro"), "<ul><li>uno</li><li>dos</li></ul><ol><li>tres</li><li>cuatro</li></ol>");
igual(
  "enlace",
  markdown("[soporte](https://soporte.ejemplo.com/a?b=1&c=2)"),
  '<p><a href="https://soporte.ejemplo.com/a?b=1&amp;c=2" target="_blank" rel="noopener noreferrer nofollow">soporte</a></p>',
);
igual(
  "dirección suelta (sin el punto final)",
  markdown("ver https://ejemplo.org/x."),
  '<p>ver <a href="https://ejemplo.org/x" target="_blank" rel="noopener noreferrer nofollow">https://ejemplo.org/x</a>.</p>',
);
igual("javascript: no se enlaza", markdown("[clic](javascript:alert(1))"), "<p>[clic](javascript:alert(1))</p>");
igual("data: no se enlaza", markdown("[x](data:text/html,hola)"), "<p>[x](data:text/html,hola)</p>");
igual("HTML crudo, escapado", markdown('<img src=x onerror="alert(1)"><script>alert(1)</script>'), "<p>&lt;img src=x onerror=&quot;alert(1)&quot;&gt;&lt;script&gt;alert(1)&lt;/script&gt;</p>");
igual(
  "comillas en el enlace no rompen el atributo",
  markdown('[a](https://e.org/"onmouseover="x)'),
  '<p>[a](<a href="https://e.org/" target="_blank" rel="noopener noreferrer nofollow">https://e.org/</a>&quot;onmouseover=&quot;x)</p>',
);
igual("negrita dentro del enlace", markdown("[**a**](https://e.org)"), '<p><a href="https://e.org" target="_blank" rel="noopener noreferrer nofollow"><strong>a</strong></a></p>');
igual("los marcadores internos no se cuelan", markdown("a\u00000\u0000b"), "<p>a0b</p>");
cierto("enlaces seguros", enlaceSeguro("https://a.b") && enlaceSeguro("mailto:x@ejemplo.com") && !enlaceSeguro("javascript:x") && !enlaceSeguro("file:///c:/x") && !enlaceSeguro("/relativo"));
igual("primera línea sin marcas", primeraLinea("\n## **Disco** nuevo\nmás"), "Disco nuevo");
igual("primera línea: enlace y lista", primeraLinea("- llamar a [Luis](https://e.org) si _falla_"), "llamar a Luis si falla");
igual("primera línea: fecha al principio", primeraLinea("3/10 cambié el disco"), "3/10 cambié el disco");
igual("primera línea, recortada", primeraLinea("a".repeat(100)).length, 80);

console.log(`\n${total - fallos} de ${total} comprobaciones correctas.`);
if (fallos) process.exit(1);
