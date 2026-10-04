; Lo común de los instaladores de Windows (servidor.nsi y agente.nsi): la
; imagen y el tono. Se incluye después de definir PRODUCT y antes de las páginas.
;
; Imagen: packaging/windows/arte/ (SVG y lo que sale de ellos con
; «npm run arte:instaladores»): el icono de varias medidas para el
; instalador, el desinstalador y «Aplicaciones instaladas», la cabecera
; (150×57) y la imagen lateral de las páginas de bienvenida y final
; (164×314), las dos también al doble para pantallas con escala.
;
; Tono: de tú, como la consola. Se cambian los textos de Modern UI que
; tratan de usted («Por favor espere…», «¿Está seguro…?»).

; makensis trabaja en la carpeta del script (sin /NOCD): rutas relativas a ella.
!define ARTE "arte"

!define MUI_ICON "${ARTE}\resguardo.ico"
!define MUI_UNICON "${ARTE}\resguardo.ico"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_RIGHT
!define MUI_HEADERIMAGE_BITMAP "${ARTE}\cabecera.bmp"
!define MUI_WELCOMEFINISHPAGE_BITMAP "${ARTE}\lateral.bmp"
!define MUI_CUSTOMFUNCTION_GUIINIT ArteAltaResolucion
!define MUI_CUSTOMFUNCTION_UNGUIINIT un.ArteAltaResolucion
!define MUI_ABORTWARNING
!define MUI_ABORTWARNING_TEXT "¿Salir de la instalación de ${PRODUCT}?"

; Con la escala de Windows al 125 % o más, las imágenes al doble de tamaño:
; Modern UI las ajusta a su sitio y se ven nítidas en vez de estiradas.
!macro ArteAltaResolucionFn un
Function ${un}ArteAltaResolucion
  System::Call 'user32::GetDpiForWindow(p $HWNDPARENT) i .r0'
  IntCmp $0 120 0 fin 0
    File "/oname=$PLUGINSDIR\modern-header.bmp" "${ARTE}\cabecera@2x.bmp"
    SetBrandingImage /IMGID=1046 /RESIZETOFIT "$PLUGINSDIR\modern-header.bmp"
    !if "${un}" == ""
      File "/oname=$PLUGINSDIR\modern-wizard.bmp" "${ARTE}\lateral@2x.bmp"
    !endif
  fin:
FunctionEnd
!macroend

; La página de instalación con los textos de tú.
!macro PaginaInstalar
  !define MUI_PAGE_HEADER_TEXT "Instalando"
  !define MUI_PAGE_HEADER_SUBTEXT "Un momento: se está instalando ${PRODUCT}."
  !define MUI_INSTFILESPAGE_FINISHHEADER_TEXT "Instalado"
  !define MUI_INSTFILESPAGE_FINISHHEADER_SUBTEXT "${PRODUCT} ya está instalado y en marcha."
  !insertmacro MUI_SET MUI_INSTFILESPAGE_ABORTHEADER_TEXT "No se ha podido instalar"
  !insertmacro MUI_SET MUI_INSTFILESPAGE_ABORTHEADER_SUBTEXT "El motivo está en la lista de abajo."
  !insertmacro MUI_PAGE_INSTFILES
!macroend

; Las páginas del desinstalador.
!macro PaginasDesinstalar
  !define MUI_PAGE_HEADER_TEXT "Desinstalar ${PRODUCT}"
  !define MUI_PAGE_HEADER_SUBTEXT "Quitarlo de este equipo."
  !define MUI_UNCONFIRMPAGE_TEXT_TOP "Se quitará ${PRODUCT} de esta carpeta. Las copias de seguridad no se tocan. Pulsa «Desinstalar» para continuar."
  !insertmacro MUI_UNPAGE_CONFIRM
  !define MUI_PAGE_HEADER_TEXT "Desinstalando"
  !define MUI_PAGE_HEADER_SUBTEXT "Un momento: se está quitando ${PRODUCT}."
  !define MUI_INSTFILESPAGE_FINISHHEADER_TEXT "Desinstalado"
  !define MUI_INSTFILESPAGE_FINISHHEADER_SUBTEXT "${PRODUCT} ya no está en este equipo."
  !insertmacro MUI_SET MUI_INSTFILESPAGE_ABORTHEADER_TEXT "No se ha podido desinstalar"
  !insertmacro MUI_SET MUI_INSTFILESPAGE_ABORTHEADER_SUBTEXT "El motivo está en la lista de abajo."
  !insertmacro MUI_UNPAGE_INSTFILES
!macroend

; Abre una dirección en el navegador con el usuario de la sesión: el
; instalador corre como administrador y el navegador no debe heredarlo
; (el Explorador la abre con su usuario).
!macro AbrirEnElNavegador url
  Exec '"$WINDIR\explorer.exe" "${url}"'
!macroend
