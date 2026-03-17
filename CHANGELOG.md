# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project largely adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-03-17

### Added

- CLI/TUI: Let users specify the color theme of the TUI
- TUI: Allow infinte backlog with smooth scrolling
- Tests: Added simple TUI rendering tests, including behaviour based on user input
- UX: Concretize user instructions when working with TUN interfaces

### Fixed

- Fix: Pass known diagnostic commands with their arguments
- Fix: Clear user input after commiting (request TUI re-rendering)

### Changed

- Chore: Internal refactoring, depenendy updates, major performance improvements
- CLI: Clarify usage & descriptions of headless modes
- CoAP: Try to display undeclared payloads as UTF-8, fall back to hex dump
- TUI: Prettify packet log display

## [0.1.1] - 2026-01-13

### Added

- CoAP: Respond to requests with 4.04 rather than ignoring them
- CoAP: Respond to separate responses and other with RST
- TUI: Respect the users terminal theme (light- / darkmode) in choosen TUI colors
- TUI: Prints one of the IP addresses (if any) of the attached network interface

### Fixed

- Networking: Can attach to existing interfaces

### Removed

- Networking: Jelly can no longer create tun interfaces

## [0.1.0] - 2025-12-24

Initial release.