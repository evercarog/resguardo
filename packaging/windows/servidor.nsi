; Instalador de Resguardo Server para Windows (consola web y canal de los agentes).
;
; Lo genera scripts/build-servidor.mjs (npm run build:servidor) con:
;   makensis /INPUTCHARSET UTF8 /DVERSION=… /DSERVER_EXE=… /DSERVER_SHA256=… /DOUT_FILE=… servidor.nsi
;
; Instala resguardo-server.exe (con la consola dentro) en Archivos de programa,
; el servicio «ResguardoServer» (arranque automático, se reinicia si falla), su
; carpeta de datos en ProgramData (solo SYSTEM y Administradores) y una regla
; del cortafuegos solo para su puerto. Al terminar muestra la dirección de la
; consola y el código de primer arranque.
;
; Sin ventanas:  Resguardo-Server_x.y.z_x64-setup.exe /S [/PUERTO=8443] [/TODALARED] [/CONAGENTE]
;
; /CONAGENTE (o la casilla «Este equipo también guarda copias»): instala
; también el agente que trae (sin vincular). La consola lo ofrece después
; como «Vincular este servidor» (docs/api-servidor.md, v1.19).
;
; Imagen y textos comunes con el de Resguardo Agente: comun.nsh y arte/.
;
; Vista previa (solo para ver las páginas y hacer capturas; nunca se publica):
;   makensis /INPUTCHARSET UTF8 /DVISTA_PREVIA=nuevo|actualizacion [/DAGENT_SETUP=x] /DOUT_FILE=… servidor.nsi
; No pide ser administrador ni instala nada: la página de instalación solo
; rellena datos de ejemplo.

Unicode true
ManifestDPIAware true
!ifdef VISTA_PREVIA
  RequestExecutionLevel user
!else
  RequestExecutionLevel admin
!endif
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!include "nsDialogs.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "x64.nsh"
!include "WinVer.nsh"
!include "StrFunc.nsh"
${StrStr}

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!define PRODUCT "Resguardo Server"
!define SERVICE "ResguardoServer"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\ResguardoServer"

Name "${PRODUCT}"
OutFile "${OUT_FILE}"
InstallDir "$PROGRAMFILES64\Resguardo Server"
BrandingText "${PRODUCT} ${VERSION}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1034 "ProductName" "${PRODUCT}"
VIAddVersionKey /LANG=1034 "FileDescription" "Instalar ${PRODUCT}"
VIAddVersionKey /LANG=1034 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1034 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1034 "LegalCopyright" "© 2026 Ever Caro y colaboradores · AGPL-3.0-or-later"

!include "comun.nsh"
!insertmacro ArteAltaResolucionFn ""
!insertmacro ArteAltaResolucionFn "un."

Var Puerto
Var PuertoInput
Var SoloLocal
Var SoloLocalBox
; «Este equipo también guarda copias»: instala también el agente que trae.
Var ConAgente
Var ConAgenteBox
Var Codigo
Var Equipo
Var BigFont
; La casilla «Abrir la consola ahora» de la página final.
Var AbrirBox

!define MUI_WELCOMEPAGE_TITLE "Instalar ${PRODUCT}"
!define MUI_WELCOMEPAGE_TEXT "Resguardo Server es la consola web desde la que se gestionan las copias de seguridad de los equipos con Resguardo Agente.$\r$\n$\r$\nSe instala como un servicio de Windows que arranca solo, y la consola se abre desde el navegador de cualquier equipo de la red.$\r$\n$\r$\nNo guarda copias ni contraseñas de los repositorios: eso se queda en cada equipo."
!insertmacro MUI_PAGE_WELCOME
Page custom OpcionesPage OpcionesLeave
!insertmacro PaginaInstalar
; Al terminar: la dirección de la consola, el código de primer arranque y «Abrir la consola ahora».
!define MUI_PAGE_CUSTOMFUNCTION_SHOW ListoShow
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE ListoLeave
!define MUI_FINISHPAGE_TITLE "Resguardo Server está en marcha"
!define MUI_FINISHPAGE_TEXT " "
; Reemplazar el programa no necesita reiniciar (el anterior se aparta y se borra al reiniciar).
!define MUI_FINISHPAGE_NOREBOOTSUPPORT
!insertmacro MUI_PAGE_FINISH

!insertmacro PaginasDesinstalar

!insertmacro MUI_LANGUAGE "Spanish"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "Resguardo Server necesita Windows de 64 bits." /SD IDOK
    SetErrorLevel 5
    Abort
  ${EndIf}
  ${IfNot} ${AtLeastWin10}
    MessageBox MB_ICONSTOP "Resguardo Server necesita Windows 10 o Windows Server 2016, o una versión posterior.$\r$\n$\r$\nEste equipo tiene una versión de Windows más antigua: actualízala y vuelve a abrir el instalador." /SD IDOK
    SetErrorLevel 5
    Abort
  ${EndIf}
  SetRegView 64
  StrCpy $Puerto "8443"
  StrCpy $SoloLocal "1"
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/PUERTO=" $R1
  ${IfNot} ${Errors}
    Push $R1
    Call PuertoValido
    Pop $R2
    ${If} $R2 != "1"
      MessageBox MB_ICONSTOP "El puerto «$R1» no es válido: escribe un número entre 1 y 65535 (por ejemplo, /PUERTO=8443)." /SD IDOK
      SetErrorLevel 6
      Abort
    ${EndIf}
    StrCpy $Puerto $R1
  ${EndIf}
  ClearErrors
  ${GetOptions} $R0 "/TODALARED" $R1
  ${IfNot} ${Errors}
    StrCpy $SoloLocal "0"
  ${EndIf}
  ; /CONAGENTE: este equipo también guarda copias (instala el agente que trae).
  StrCpy $ConAgente "0"
  ClearErrors
  ${GetOptions} $R0 "/CONAGENTE" $R1
  ${IfNot} ${Errors}
    StrCpy $ConAgente "1"
  ${EndIf}
  ReadEnvStr $Equipo "COMPUTERNAME"
FunctionEnd

; ¿Es un puerto válido? Solo cifras, sin ceros delante (NSIS los leería en
; octal) y entre 1 y 65535. Deja "1" o "0" en la pila.
Function PuertoValido
  Exch $R0
  Push $R1
  Push $R2
  Push $R3
  Push $R4
  StrLen $R1 $R0
  StrCpy $R3 "1"
  ${If} $R1 < 1
  ${OrIf} $R1 > 5
    StrCpy $R3 "0"
  ${EndIf}
  StrCpy $R2 0
  ${DoWhile} $R2 < $R1
    StrCpy $R4 $R0 1 $R2
    ${If} $R4 S< "0"
    ${OrIf} $R4 S> "9"
      StrCpy $R3 "0"
    ${EndIf}
    IntOp $R2 $R2 + 1
  ${Loop}
  StrCpy $R4 $R0 1
  ${If} $R4 == "0"
    StrCpy $R3 "0"
  ${EndIf}
  ${If} $R3 == "1"
  ${AndIf} $R0 > 65535
    StrCpy $R3 "0"
  ${EndIf}
  StrCpy $R0 $R3
  Pop $R4
  Pop $R3
  Pop $R2
  Pop $R1
  Exch $R0
FunctionEnd

; ---------- Opciones ----------

Function OpcionesPage
  !insertmacro MUI_HEADER_TEXT "Opciones" "Dónde escucha la consola."
  nsDialogs::Create 1018
  Pop $0
  ${NSD_CreateLabel} 0 0 100% 12u "Puerto de la consola (HTTPS):"
  Pop $0
  ${NSD_CreateNumber} 0 14u 60u 13u $Puerto
  Pop $PuertoInput
  ${NSD_CreateCheckbox} 0 40u 100% 12u "Solo redes internas (recomendado)"
  Pop $SoloLocalBox
  ${If} $SoloLocal == "1"
    ${NSD_Check} $SoloLocalBox
  ${EndIf}
  ${NSD_CreateLabel} 0 58u 100% 40u "Se crea una regla del cortafuegos de Windows solo para ese puerto. Con «Solo redes internas», la consola se puede abrir desde los equipos de la empresa (también desde otras subredes o VLAN), pero no desde internet. Nunca se abre ningún puerto en el router."
  Pop $0
  !ifdef AGENT_SETUP
    ${NSD_CreateCheckbox} 0 98u 100% 12u "Este equipo también guarda copias (instala Resguardo Agente)"
    Pop $ConAgenteBox
    ${If} $ConAgente == "1"
      ${NSD_Check} $ConAgenteBox
    ${EndIf}
    ${NSD_CreateLabel} 12u 111u 95% 28u "Para usar su disco como almacén de los demás equipos. Después, en la consola, «Vincular este servidor» lo da de alta con la clave de administración y eliges la carpeta."
    Pop $0
  !endif
  nsDialogs::Show
FunctionEnd

Function OpcionesLeave
  ${NSD_GetText} $PuertoInput $Puerto
  Push $Puerto
  Call PuertoValido
  Pop $0
  ${If} $0 != "1"
    MessageBox MB_ICONEXCLAMATION "Escribe un puerto entre 1 y 65535 (por ejemplo, 8443)."
    Abort
  ${EndIf}
  ${NSD_GetState} $SoloLocalBox $0
  ${If} $0 == ${BST_CHECKED}
    StrCpy $SoloLocal "1"
  ${Else}
    StrCpy $SoloLocal "0"
  ${EndIf}
  !ifdef AGENT_SETUP
    ${NSD_GetState} $ConAgenteBox $0
    ${If} $0 == ${BST_CHECKED}
      StrCpy $ConAgente "1"
    ${Else}
      StrCpy $ConAgente "0"
    ${EndIf}
  !endif
FunctionEnd

; ---------- Instalación ----------

Section "Resguardo Server" SecMain
  SectionIn RO
  SetShellVarContext all
  SetRegView 64
!ifdef VISTA_PREVIA
  ; Solo para ver las páginas: datos de ejemplo, nada instalado.
  DetailPrint "Vista previa: no se instala nada."
  StrCpy $Codigo ""
  StrCpy $Equipo "SERVIDOR-COPIAS"
  !if "${VISTA_PREVIA}" == "nuevo"
    StrCpy $Codigo "7KQ2-M9XD-4FPT"
  !endif
  DetailPrint "Consola: https://$Equipo:$Puerto/"
!else

  ; Actualización: parar el servicio antes de reemplazar el programa.
  DetailPrint "Parando el servicio (si estaba en marcha)..."
  nsExec::Exec '"$SYSDIR\sc.exe" stop ${SERVICE}'
  Pop $0
  Sleep 2000
  nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM resguardo-server.exe'
  Pop $0

  SetOutPath "$INSTDIR"
  ; Si el programa anterior sigue bloqueado un momento, se aparta (renombrar
  ; funciona aunque esté en uso) para que el nuevo se escriba seguro.
  Delete "$INSTDIR\resguardo-server.exe.anterior"
  ${If} ${FileExists} "$INSTDIR\resguardo-server.exe"
    Rename "$INSTDIR\resguardo-server.exe" "$INSTDIR\resguardo-server.exe.anterior"
  ${EndIf}
  File "/oname=resguardo-server.exe" "${SERVER_EXE}"
  Delete /REBOOTOK "$INSTDIR\resguardo-server.exe.anterior"
  !ifdef SERVER_SHA256
    nsExec::ExecToStack '"$SYSDIR\certutil.exe" -hashfile "$INSTDIR\resguardo-server.exe" SHA256'
    Pop $0
    Pop $1
    ${StrStr} $2 $1 "${SERVER_SHA256}"
    ${If} $2 == ""
      MessageBox MB_ICONSTOP "El programa instalado no coincide con el de este instalador." /SD IDOK
      Delete "$INSTDIR\resguardo-server.exe"
      SetErrorLevel 3
      Abort
    ${EndIf}
  !endif

  ; El instalador del agente con el que se compiló (v1.17): la consola lo da
  ; «listo para vincular» (le añade al final los datos y un código de un solo uso).
  !ifdef AGENT_SETUP
    SetOutPath "$INSTDIR\agente"
    File "/oname=Resguardo-Agente-setup.exe" "${AGENT_SETUP}"
    SetOutPath "$INSTDIR"
  !endif

  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "Resguardo"
  ; El icono de Resguardo (resguardo-server.exe no lleva icono; el desinstalador, sí).
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\uninstall.exe,0"
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1

  DetailPrint "Instalando el servicio ${SERVICE} (puerto $Puerto)..."
  ${If} $SoloLocal == "1"
    nsExec::ExecToLog '"$INSTDIR\resguardo-server.exe" --instalar-servicio --escuchar 0.0.0.0:$Puerto'
  ${Else}
    nsExec::ExecToLog '"$INSTDIR\resguardo-server.exe" --instalar-servicio --escuchar 0.0.0.0:$Puerto --toda-la-red'
  ${EndIf}
  Pop $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "No se pudo instalar el servicio de Resguardo Server (código $0). Mira los detalles de la instalación; si el puerto está ocupado, elige otro." /SD IDOK
    SetErrorLevel 4
    Abort
  ${EndIf}

  ; El código de primer arranque (solo si aún no hay cuentas).
  StrCpy $Codigo ""
  ClearErrors
  FileOpen $1 "$APPDATA\Resguardo Server\codigo-arranque.txt" r
  ${IfNot} ${Errors}
    FileRead $1 $Codigo
    FileClose $1
  ${EndIf}
  ; Este equipo también guarda copias: el agente, sin ventanas y sin vincular.
  ; Se vincula después desde la consola («Vincular este servidor»): hace falta
  ; la cuenta, el cliente y la clave de administración, que aún no existen.
  !ifdef AGENT_SETUP
    ${If} $ConAgente == "1"
      DetailPrint "Instalando Resguardo Agente en este equipo..."
      ExecWait '"$INSTDIR\agente\Resguardo-Agente-setup.exe" /S /TRAY=0' $0
      ${If} $0 != 0
        DetailPrint "No se pudo instalar Resguardo Agente (código $0): instálalo después con $INSTDIR\agente\Resguardo-Agente-setup.exe."
      ${EndIf}
    ${EndIf}
  !endif
  DetailPrint "Consola: https://$Equipo:$Puerto/"
!endif
SectionEnd

; ---------- Listo ----------

; La página final de Modern UI (con la imagen lateral), con lo que hace falta
; para empezar: la dirección de la consola, el código de primer arranque y
; la casilla para abrirla. Columna de texto: 120u a 315u.
Function ListoShow
  ShowWindow $mui.FinishPage.Text ${SW_HIDE}
  ${NSD_CreateLabel} 120u 45u 195u 18u "Abre la consola desde el navegador de cualquier equipo de la red:"
  Pop $0
  SetCtlColors $0 "${MUI_TEXTCOLOR}" "${MUI_BGCOLOR}"
  ${NSD_CreateText} 120u 64u 195u 13u "https://$Equipo:$Puerto/"
  Pop $0
  SendMessage $0 ${EM_SETREADONLY} 1 0
  ${If} $Codigo != ""
    ${NSD_CreateLabel} 120u 84u 195u 18u "La primera vez te pedirá este código para crear la cuenta de propietario (sirve una sola vez):"
    Pop $0
    SetCtlColors $0 "${MUI_TEXTCOLOR}" "${MUI_BGCOLOR}"
    ${NSD_CreateText} 120u 104u 195u 18u "$Codigo"
    Pop $0
    CreateFont $BigFont "Segoe UI" 14 700
    SendMessage $0 ${WM_SETFONT} $BigFont 1
    SendMessage $0 ${EM_SETREADONLY} 1 0
    ${NSD_CreateLabel} 120u 126u 195u 42u "También está en $APPDATA\Resguardo Server\codigo-arranque.txt hasta que se use. El navegador avisará del certificado: es el propio del servidor (lo explica la guía)."
    Pop $0
    SetCtlColors $0 "${MUI_TEXTCOLOR}" "${MUI_BGCOLOR}"
  ${Else}
    ${NSD_CreateLabel} 120u 84u 195u 30u "Este servidor ya tenía cuentas (es una actualización): entra con la tuya."
    Pop $0
    SetCtlColors $0 "${MUI_TEXTCOLOR}" "${MUI_BGCOLOR}"
  ${EndIf}
  ${NSD_CreateCheckbox} 120u 175u 195u 10u "Abrir la consola ahora"
  Pop $AbrirBox
  SetCtlColors $AbrirBox "${MUI_TEXTCOLOR}" "${MUI_BGCOLOR}"
  ${NSD_Check} $AbrirBox
  ${NSD_SetFocus} $AbrirBox
FunctionEnd

Function ListoLeave
  ${NSD_GetState} $AbrirBox $0
  ${If} $0 == ${BST_CHECKED}
    !insertmacro AbrirEnElNavegador "https://$Equipo:$Puerto/"
  ${EndIf}
FunctionEnd

; ---------- Desinstalación ----------

Section "Uninstall"
  SetShellVarContext all
  SetRegView 64
  DetailPrint "Parando y quitando el servicio..."
  nsExec::ExecToLog '"$INSTDIR\resguardo-server.exe" --desinstalar-servicio'
  Pop $0
  nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM resguardo-server.exe'
  Pop $0

  ; Los datos (cuentas, clientes, equipos, auditoría y la identidad del
  ; servidor que fijan los agentes) se conservan salvo que se pida lo contrario.
  ${If} ${FileExists} "$APPDATA\Resguardo Server\*.*"
    MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "¿Borrar también los datos de Resguardo Server?$\r$\n$\r$\n$APPDATA\Resguardo Server guarda las cuentas, los clientes, los equipos, la auditoría y la identidad del servidor. Si la borras, los agentes tendrán que volver a vincularse.$\r$\n$\r$\nLas copias de seguridad no están aquí y no se tocan.$\r$\n$\r$\nSi vas a volver a instalarlo, elige «No»." /SD IDNO IDNO conservar
    DetailPrint "Borrando los datos del servidor..."
    RMDir /r "$APPDATA\Resguardo Server"
    conservar:
  ${EndIf}

  Delete "$INSTDIR\resguardo-server.exe"
  Delete "$INSTDIR\agente\Resguardo-Agente-setup.exe"
  RMDir "$INSTDIR\agente"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "${UNINST_KEY}"
SectionEnd
