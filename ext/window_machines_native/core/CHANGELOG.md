# Changelog

## [0.4.0](https://github.com/seuros/window_machines/compare/window-machines-v0.3.0...window-machines-v0.4.0) (2026-11-14)


### ⚠ BREAKING CHANGES

* seal window values, check exact classes natively, re-check frozen before keeping

### Features

* seal window values, check exact classes natively, re-check frozen before keeping ([9a891df](https://github.com/seuros/window_machines/commit/9a891dfdb502048734b2a4db99823e800ff11e81))

## [0.3.0](https://github.com/seuros/window_machines/compare/window-machines-v0.2.2...window-machines-v0.3.0) (2026-11-13)


### ⚠ BREAKING CHANGES

* converted to the fleet conventions. One exhaustive Refusal per crate, Result from every operation that can refuse, a refusal changes nothing, provable idempotent repeats are honoured, Ruby mirrors the core. No compatibility aliases. A late event inside a still-held session is Covered; Ruby refusals are symbols and no longer raise.

### Features

* answer refusals as Result&lt;_, Refusal&gt;, honour repeats, write sliding windows into a caller buffer ([520e126](https://github.com/seuros/window_machines/commit/520e126842a5065411eae7dafed05151da2919ff))

## [0.2.2](https://github.com/seuros/window_machines/compare/window-machines-v0.2.1...window-machines-v0.2.2) (2026-11-06)


### Bug Fixes

* crates.io keywords and categories for the core ([b8098d7](https://github.com/seuros/window_machines/commit/b8098d7d280dfd4e48ba7ade3a72b7f891853ec4))

## [0.2.1](https://github.com/seuros/window_machines/compare/window-machines-v0.2.0...window-machines-v0.2.1) (2026-11-06)


### Bug Fixes

* keep test sources out of the packaged gem and crate ([db7ff81](https://github.com/seuros/window_machines/commit/db7ff817e11a2448c17640631c2f8a6a254447a3))

## [0.2.0](https://github.com/seuros/window_machines/compare/window-machines-v0.1.0...window-machines-v0.2.0) (2026-11-06)


### Features

* initial extraction ([3bd753e](https://github.com/seuros/window_machines/commit/3bd753ec5ae5890e4e54272e5df4523c9d685d4b))
