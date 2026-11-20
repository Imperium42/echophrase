; Echophrase NSIS Installer Hooks
; No custom install hooks needed - DirectML handles Windows GPU acceleration natively.
;
; CUDA support (if desired) should be detected at runtime from system-wide installation,
; not downloaded during install. Users who want CUDA can install it via NVIDIA's toolkit.

!macro NSIS_HOOK_POSTINSTALL
  ; No post-install actions needed
!macroend

!macro NSIS_HOOK_PREINSTALL
  ; No pre-install actions needed
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; No pre-uninstall actions needed
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; No post-uninstall actions needed
!macroend
