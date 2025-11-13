# markadd Documentation

Welcome to the documentation for **markadd**, a terminal-first Markdown automation tool inspired by Obsidian’s QuickAdd.

This documentation includes:

- configuration reference  
- architecture notes  
- development logs  
- roadmap overview  

If you're new, begin with:

`docs/CONFIG.md`

For development progress, see:

`docs/devlogs/`

## Contents

### Configuration
docs/CONFIG.md

### Development Logs
docs/devlogs/phase-01-dev-log.md  
docs/devlogs/phase-02-dev-log.md  
(and future phases)

### Source Layout

crates/core   – configuration, template discovery, engines  
crates/cli    – command-line interface  
crates/tui    – reserved for future terminal UI

## Philosophy

`markadd` aims to be:

- deterministic and testable  
- terminal-native  
- Markdown-first  
- extensible through templates, captures, and macros  

Return to project root:

../README.md
