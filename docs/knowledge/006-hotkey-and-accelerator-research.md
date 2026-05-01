---
title: Hotkey And Accelerator Research
status: abandoned
tags: [windows, hotkeys, accelerators, research]
---

# Hotkey And Accelerator Research

## Summary

Two Windows hotkey and accelerator research paths were explored but are not
currently useful enough to drive implementation.

## HWND Accelerator Keys

Question explored: can Omni Palette get all accelerator keys available from a
target `HWND`?

Finding: partial discovery appears possible, but not all accelerator keys are
available through the attempted approach.

Current status: abandoned.

## Hotkey Registration Conflicts

Question explored: can Omni Palette determine whether AutoHotkey, PowerToys, or
another tool registered a global hotkey first?

Finding: there does not appear to be a reliable way to tell which tool registered
the command first at the OS level.

Current status: keep this as a constraint when designing interceptor or conflict
tooling.
