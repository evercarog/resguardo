; Ganchos del instalador NSIS de Resguardo (convención de Tauri:
; NSIS_HOOK_POSTINSTALL / NSIS_HOOK_PREUNINSTALL).

; Al terminar de instalar o actualizar: la desinstalación de la versión
; anterior quitó la tarea del agente; se vuelve a crear si este equipo tiene
; copias programadas o está vinculado a la web (si no, no hace nada).
!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Reactivando el agente de copias (si estaba configurado)..."
  ExecWait '"$INSTDIR\resguardo.exe" --agent-sync-task' $0
  DetailPrint "Agente: código $0"
!macroend

; Antes de desinstalar (también al actualizar, que desinstala la versión
; anterior con /UPDATE):
; 1. Siempre: quitar la tarea del agente, para que no quede ejecutándose sin
;    programa. Al actualizar, la nueva versión la vuelve a crear.
; 2. Solo al desinstalar de verdad (no al actualizar ni en modo pasivo o
;    silencioso): preguntar si también se borran los datos del agente
;    (%ProgramData%\Resguardo: programación, historial y contraseñas cifradas).
;    Por defecto se conservan. El borrado lo hace resguardo.exe --agent-purge
;    antes de quitar el programa: no sigue enlaces (si la carpeta fuera una
;    unión, solo quita la unión, nunca lo que hay en su destino).
!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec '"$SYSDIR\schtasks.exe" /Delete /TN "Resguardo\Agente" /F'
  Pop $0
  ; Servidor de copias: se para y se quita su tarea (al actualizar, la versión
  ; nueva la vuelve a crear si estaba activado). Sin el programa, también su
  ; regla del firewall.
  nsExec::Exec '"$INSTDIR\resguardo.exe" --server-stop'
  Pop $0
  nsExec::Exec '"$SYSDIR\schtasks.exe" /End /TN "Resguardo Servidor de copias"'
  Pop $0
  nsExec::Exec '"$SYSDIR\schtasks.exe" /Delete /TN "Resguardo Servidor de copias" /F'
  Pop $0
  ${If} $UpdateMode <> 1
    nsExec::Exec '"$SYSDIR\netsh.exe" advfirewall firewall delete rule name="Resguardo Servidor de copias"'
    Pop $0
  ${EndIf}

  ; Al desinstalar de verdad (no al actualizar): quitar lo que la app pone para
  ; el usuario que desinstala: el inicio con Windows y «Ver versiones en
  ; Resguardo» del menú del Explorador (apuntarían a un programa que ya no está).
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Resguardo"
    DeleteRegKey HKCU "Software\Classes\*\shell\ResguardoVersiones"
    DeleteRegKey HKCU "Software\Classes\Directory\shell\ResguardoVersiones"
  ${EndIf}

  ${If} $UpdateMode <> 1
  ${AndIf} $PassiveMode <> 1
    SetShellVarContext all
    ${If} ${FileExists} "$APPDATA\Resguardo\*.*"
      MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "¿Borrar también los datos de las copias automáticas de este equipo?$\r$\n$\r$\n$APPDATA\Resguardo guarda la programación, el historial y las contraseñas (cifradas) que usa el agente. Las copias guardadas en tus destinos no se tocan.$\r$\n$\r$\nSi vas a volver a instalar Resguardo, elige «No» para conservarlos." /SD IDNO IDNO resguardo_keep_agent_data
      DetailPrint "Borrando los datos del agente..."
      nsExec::Exec '"$INSTDIR\resguardo.exe" --agent-purge'
      Pop $0
      DetailPrint "Datos del agente: código $0"
      resguardo_keep_agent_data:
    ${EndIf}
  ${EndIf}
!macroend
