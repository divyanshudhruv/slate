# Changelog

> Entries below are generated from past commits and describe behaviour that has since been removed: the global keyboard hook, the HMAC-signed janitor payload, and the self-swap update helper.
> Shortcuts now use `RegisterHotKey`, crash recovery is plain JSON, and Slate only checks for newer releases instead of installing them.

## [0.5.1](https://github.com/divyanshudhruv/slate/compare/v0.5.0...v0.5.1) (2026-10-29)


### Bug Fixes

* **update:** replace the batch swap helper with --swap-helper ([d50db61](https://github.com/divyanshudhruv/slate/commit/d50db61abce1c7fb87a0c2bf356efd4aaee16b90))
* **win:** harden recovery, hook lifetime, and pick frame ([1f39d58](https://github.com/divyanshudhruv/slate/commit/1f39d58f81f48c430e03770fbea8c6a02d190779))


### Refactoring

* **install:** remove the start-menu shortcut ([402fe2e](https://github.com/divyanshudhruv/slate/commit/402fe2e93192bd22cdeb50fb8de7910529806bfc))


### Documentation

* **readme:** unwrap every sentence to one line ([138ac04](https://github.com/divyanshudhruv/slate/commit/138ac04dd4f74a128c8938c3dc060459c9970c46))

## [0.5.0](https://github.com/divyanshudhruv/slate/compare/v0.4.0...v0.5.0) (2026-10-28)


### Features

* **config:** per-app host rules ([f121a6f](https://github.com/divyanshudhruv/slate/commit/f121a6f23fe7ba1b2b30e52a61afa15916d0aaf5))
* **diagnostics:** copy-diagnostics bundle ([999acaf](https://github.com/divyanshudhruv/slate/commit/999acafc1ea8bf8ca34885b09ac2928b221e70be))
* **keybinds:** configurable shortcuts, keybind dialog, hover preview ([7ea2cd9](https://github.com/divyanshudhruv/slate/commit/7ea2cd9fbe7787160c14d807419a8d03d1b3c047))
* **keys:** numpad and media-key bindings ([fcfbd96](https://github.com/divyanshudhruv/slate/commit/fcfbd96a309a126082aee3d4753a0555ff90c9f1))
* **picker:** merge pick and add into one row ([8d4e52f](https://github.com/divyanshudhruv/slate/commit/8d4e52f09a884cef08aaf398f2d2c6256c10eca0))
* **picker:** move the row highlight with arrow keys and pick with Enter ([38ac8f7](https://github.com/divyanshudhruv/slate/commit/38ac8f7f70763fe2d00b22448ba5b42b35732216))
* **picker:** search windows and add several at once ([b9edde2](https://github.com/divyanshudhruv/slate/commit/b9edde2f96e8ab673ce6392d61b7c4d6dd11cd7f))
* **session:** crash recovery prompt from janitor payload ([60f207b](https://github.com/divyanshudhruv/slate/commit/60f207b6af790f16a1fe01d64e00449a9168b16c))
* **session:** restore tabs on launch ([65bb74b](https://github.com/divyanshudhruv/slate/commit/65bb74badb40660dcfa29a8b005bcc40fde4f54e))
* **sets:** import and export saved sets ([ee26d66](https://github.com/divyanshudhruv/slate/commit/ee26d66fb909229799a434875b03d53b35b864a6))
* **sets:** match saved tabs by process and placement ([e3525c1](https://github.com/divyanshudhruv/slate/commit/e3525c1a6151d26a4e54e0e8a8b3798365a4c820))
* **theme:** cycle system, dark, and light ([f3918d4](https://github.com/divyanshudhruv/slate/commit/f3918d48a41d490e5258216ab1d4102f2368b199))
* **ui:** drive the tab and saved-set menus from the keyboard ([85314a4](https://github.com/divyanshudhruv/slate/commit/85314a47c30cea9c34bf0566da560c55522ad67c))
* **ui:** keyboard navigation for the all-tabs overflow menu ([b8c4392](https://github.com/divyanshudhruv/slate/commit/b8c43925b35915d1b40f78b7bb3302f76cbc556e))
* **ui:** one banner strip, real update progress, and a flat menu ([b7ccb60](https://github.com/divyanshudhruv/slate/commit/b7ccb6095d410e13d6bafb1c00abb5b77eaabb4f))
* **update:** beta channel and forced check ([0e5b6d7](https://github.com/divyanshudhruv/slate/commit/0e5b6d78e492b23122046f42e8b972ea2e158ff9))


### Bug Fixes

* **app:** restore owned windows before exit ([5143624](https://github.com/divyanshudhruv/slate/commit/51436240b8641f84a487146e36cb20409bdfa346))
* **capture:** fall back to PrintWindow on a flat screen grab ([cfb0f62](https://github.com/divyanshudhruv/slate/commit/cfb0f62085411c86b0c650a61b81e0e382a757fe))
* **config:** keep settings when one field is malformed ([a5e6759](https://github.com/divyanshudhruv/slate/commit/a5e6759a266d0965fe8e7602f679710aa481d0ec))
* **config:** single data root, portable-aware paths, correct folder open ([89ef9c8](https://github.com/divyanshudhruv/slate/commit/89ef9c888d9115a59b6e988409fcef3aca6a5a3b))
* **config:** surface save failures instead of ignoring io errors ([702051e](https://github.com/divyanshudhruv/slate/commit/702051e498186083e5cdc1fb68d5a0cb2df31887))
* **deps:** bump pulldown-cmark to 0.13.5 for RUSTSEC-2026-0414 ([3e916b8](https://github.com/divyanshudhruv/slate/commit/3e916b8d4350495e51d20fbe876f95663915d350))
* **host:** event-driven member sync with idle backoff ([96b53fa](https://github.com/divyanshudhruv/slate/commit/96b53fa98416f8989fb91966e70ccebe0ac678e6))
* **host:** reliable restore, elevated hosting, and single instance ([1937c29](https://github.com/divyanshudhruv/slate/commit/1937c29597185a6aa62dfd7b635cfa9e5077c79c))
* **keys:** layout-aware names and right-to-left chord parsing ([468bb9a](https://github.com/divyanshudhruv/slate/commit/468bb9ac0682e2c6203c4aec47da1babfe0d7ea0))
* **picker:** evict icons for non-hosted windows ([5f6e0cf](https://github.com/divyanshudhruv/slate/commit/5f6e0cf5384aed9b2b58000c656209e58db2ad4a))
* **preview:** skip minimized and empty captures ([6e30477](https://github.com/divyanshudhruv/slate/commit/6e30477588eda0f16fbc9a99e6437c1609ccd99c))
* **sets:** keep tab colors with their windows ([7049301](https://github.com/divyanshudhruv/slate/commit/704930110442837daa1cc73b27b46d1f14046ccf))
* **sets:** restore tab colors by window identity ([aa2a5ff](https://github.com/divyanshudhruv/slate/commit/aa2a5ff9d22ef377d4a61779da3b33db51772af4))
* **settings:** add Reload settings and tint palettes with the accent ([16bf9a0](https://github.com/divyanshudhruv/slate/commit/16bf9a077d4669293cae5903596cceaa52d18b86))
* **shortcut:** debounce key repeat ([164f38d](https://github.com/divyanshudhruv/slate/commit/164f38dd76a71d66f0e8d87ffe3dd30031cb62ed))
* **shortcut:** grow owned-window set instead of truncating ([be5f773](https://github.com/divyanshudhruv/slate/commit/be5f773ed604987c0b7cca2cdfde658ef78f32c2))
* **shortcut:** make the keyboard hook lock-free ([b285143](https://github.com/divyanshudhruv/slate/commit/b2851437ab67345a718be26961fa62622dfc5339))
* **ui:** default hover preview off and restore the empty state ([27de3fe](https://github.com/divyanshudhruv/slate/commit/27de3fee9fbcb64ae8104d5c49149403292e2ccd))
* **ui:** draw hover preview in a topmost window ([a538f1d](https://github.com/divyanshudhruv/slate/commit/a538f1d5588be2907f2951739f31b547b4e05c00))
* **ui:** neutral accent, correct icon colours, and snappier shortcuts ([9b0b75a](https://github.com/divyanshudhruv/slate/commit/9b0b75a5259134c865eda20edabb973000ff05cd))
* **ui:** render the recovery prompt as a dialog and fix the overlay heights ([953db9a](https://github.com/divyanshudhruv/slate/commit/953db9ad23a41a7f2460ac884a5d264a34b14b11))
* **ui:** show icon of active theme ([4d00988](https://github.com/divyanshudhruv/slate/commit/4d00988654fb3e2fe75f535ca9029ad42969c98b))
* **update:** beta ordering, launch rollback, and quiet checks ([7389ad8](https://github.com/divyanshudhruv/slate/commit/7389ad88fdad33769cd75e658650ba0b2ea55bdc))
* **update:** keep the rollback copy and reject malformed versions ([67db1fc](https://github.com/divyanshudhruv/slate/commit/67db1fcf832f9bfc8d63c4ffa2871ecd89c0e2fe))
* **update:** lengthen cooldown and honour rate limit headers ([ae685ac](https://github.com/divyanshudhruv/slate/commit/ae685acf3e5c2cf0cf7d65ff807bfdb7f7511ef2))
* **update:** only surface an update once its assets are uploaded ([9810b81](https://github.com/divyanshudhruv/slate/commit/9810b81a0b5ce5a93aed981de9cd5af9d424cf5c))
* **win:** bound capture retries and correct pixel channel order ([0a58a55](https://github.com/divyanshudhruv/slate/commit/0a58a5552185af7098a9c9d48583a71d5f668faf))
* **win:** correct the message box, free GDI and COM handles, and steady the instance lock ([43f5910](https://github.com/divyanshudhruv/slate/commit/43f59109030075e1a7bf41ca077ef40d47f8b101))
* **win:** secure janitor state, harden picking, roll back swaps ([d41d4be](https://github.com/divyanshudhruv/slate/commit/d41d4be22278354f79055f7b76bff8313b9c8351))


### Performance

* **host:** cache window regions instead of rebuilding each move ([9f16b8f](https://github.com/divyanshudhruv/slate/commit/9f16b8f1687b0c80a685182330df53b5dd83ed87))
* **update:** move blocking http off the gpui executor ([a58f528](https://github.com/divyanshudhruv/slate/commit/a58f52836e19b160c01796683912460b0f86204a))


### Refactoring

* **config:** derive settings deserialization ([de4a2c3](https://github.com/divyanshudhruv/slate/commit/de4a2c3732d1ac22473b2dca9d6ed719b6aa70d6))
* **config:** typed theme and accent enums ([b11c390](https://github.com/divyanshudhruv/slate/commit/b11c3905faaf94359e911c26461ec1304cb5b4a7))
* **core:** split the controller state and unify config parsing ([fc2e223](https://github.com/divyanshudhruv/slate/commit/fc2e2236b69464f862574593021bedda718a2175))
* **sets:** remove saved-set import and export ([b02faae](https://github.com/divyanshudhruv/slate/commit/b02faae2136bea60fc32e0932a63068da882dcbd))
* **ui:** share row and anchor builders and split the host render ([6f707f9](https://github.com/divyanshudhruv/slate/commit/6f707f96a530ec3a20314244d30b10dd03a3adeb))


### Documentation

* **readme:** document picker keys, set matching, and reload settings ([ac8238c](https://github.com/divyanshudhruv/slate/commit/ac8238c8323f7d6abecddb648e83b07922c5a10c))
* **security:** add security model ([f40662e](https://github.com/divyanshudhruv/slate/commit/f40662e3bd9d96edd5c7523cb4da4fbedfa8cd72))

## [0.4.0](https://github.com/divyanshudhruv/slate/compare/v0.3.0...v0.4.0) (2026-10-24)


### Features

* **updates:** in-app auto-download, install with mandatory checksum verification ([f02c9bb](https://github.com/divyanshudhruv/slate/commit/f02c9bb9ac82a6cd5f20e58b9892f9a318de9ec0))


### Bug Fixes

* **ci:** allow CDLA-Permissive-2.0 and clean warnings for fmt/clippy ([11355c6](https://github.com/divyanshudhruv/slate/commit/11355c601a9230205b393b2f2b5860a95ec373bd))
* **saved-sets:** restore tab accent colors on restore/add, simplify UI, and clean up DTO ([03f7d2d](https://github.com/divyanshudhruv/slate/commit/03f7d2d9233b78cc11fd3cc55666e50b895b0807))


### Refactoring

* **saved-sets:** remove UpdateSet dead code and clean up unused helpers ([ddbc0c9](https://github.com/divyanshudhruv/slate/commit/ddbc0c92333bf15ea32c9284543f43d1a4e92226))
* **storage:** split install/data dirs and auto-migrate from .burntfern ([7f2f6cf](https://github.com/divyanshudhruv/slate/commit/7f2f6cfc4726442ad164779360add194d83ae3cb))


### Documentation

* **readme:** update list format ([44c6fd5](https://github.com/divyanshudhruv/slate/commit/44c6fd52030789af2d886b4171f6488a7a4964f6))

## [0.3.0](https://github.com/divyanshudhruv/slate/compare/v0.2.1...v0.3.0) (2026-10-18)


### Features

* **update:** add install record, bootstrap, and self-update on launch ([a92e158](https://github.com/divyanshudhruv/slate/commit/a92e1580ca503a45e8a9e7c07e2e3f2dda581411))


### Bug Fixes

* **app:** re-apply host geometry on drift and hide the janitor console ([12b92d5](https://github.com/divyanshudhruv/slate/commit/12b92d5c8a1b6bb70539d92ccd5e0c37c028a8e9))

## [0.2.1](https://github.com/divyanshudhruv/slate/compare/v0.2.0...v0.2.1) (2026-10-18)


### Bug Fixes

* **ci:** correct GitHub App token repository scope ([89e3a06](https://github.com/divyanshudhruv/slate/commit/89e3a06b1c2c03a53d1ca7dffd8d3cba5d913404))

## [0.2.0](https://github.com/burntfern/slate/compare/v0.1.0...v0.2.0) (2026-10-17)


### Features

* **update:** apply releases in place and clean up stale copies ([11ce343](https://github.com/burntfern/slate/commit/11ce343fd3b4f8cfddfbef0dac7e96b51d6891c6))


### Bug Fixes

* **update:** surface update-check results as toasts ([583ef23](https://github.com/burntfern/slate/commit/583ef237ba5b67b7a5f0ceec064dc9f0bff2950a))

## 0.1.0 (2026-10-17)


### Features

* **app:** add keyboard shortcuts, ctrl-drag tabs, and a persistent config store ([3a5aa05](https://github.com/burntfern/slate/commit/3a5aa054d628c603f7cd7fcb21c774f6139691da))
* **controller:** drive tab groups and host/member sync ([6c34bd2](https://github.com/burntfern/slate/commit/6c34bd28d8c05b0a173e41b80eb6a9b55d829ceb))
* **core:** add group state machine and workspace restore planner with tests ([46f1fda](https://github.com/burntfern/slate/commit/46f1fdab0c820721fc50fed857e230fc13218dd2))
* **tabs:** scroll overflowing tab strip and drag tabs out to detach ([1936504](https://github.com/burntfern/slate/commit/1936504408c0f34891e03680b713f379e5c18c76))
* **tray:** add system tray icon and drop app rules ([81a994c](https://github.com/burntfern/slate/commit/81a994c31981b5c571a3ef642aa8373f36c99ec3))
* **ui:** add per-tab accent colors ([315173f](https://github.com/burntfern/slate/commit/315173f0619646928d6928c87b54ad9578853691))
* **ui:** add snapshot-based tab hover preview ([86204f5](https://github.com/burntfern/slate/commit/86204f50b18ef763908be9dae116f0533324a3de))
* **ui:** add tab context menu, window picker, and ungroup/close ([6fa1110](https://github.com/burntfern/slate/commit/6fa1110d7e6964618fc15925940847436af2d748))
* **ui:** add tabbed host window shell ([db8e5e3](https://github.com/burntfern/slate/commit/db8e5e32307d92625f5d095ad14a6fb2f6bf3280))
* **update:** add update checks, version menu, and keycap empty states ([72aa6a0](https://github.com/burntfern/slate/commit/72aa6a06179df2f3d8132844ea5671bedea99863))
* **win:** add window enumeration, elevation detection, and foreground activation ([f88799c](https://github.com/burntfern/slate/commit/f88799cf1fc4f7245dd6fa5898aa8fa926e7f69c))
* **win:** add WinEvent hooks for window destroy, title, and foreground ([bffbe6a](https://github.com/burntfern/slate/commit/bffbe6a235a0ba5989e77554a7579974a3f096b4))
* **win:** host foreign windows via reparent and region crop ([83eccda](https://github.com/burntfern/slate/commit/83eccda3216ee4501f46ea37746c62d5765bccd5))


### Bug Fixes

* **persistence:** survive corrupt config and janitor state files ([9583e41](https://github.com/burntfern/slate/commit/9583e41a1e07ef6b63e5fd2d11a784e414d80453))
* **win:** stop the tab crashes and Win32 handle leaks ([a385eaf](https://github.com/burntfern/slate/commit/a385eaf7f5acd94478775d307e0afbab1579b869))


### Refactoring

* **all:** split modules and trim render work ([31fa925](https://github.com/burntfern/slate/commit/31fa925f010fc6d253b89c9e28c76bdc1d3ef7b1))
* **app:** apply audit fixes, a11y roles, CI hardening and bug fixes ([31b8019](https://github.com/burntfern/slate/commit/31b80198d3a29af379320ae80d6dd0ada815be76))
* **controller:** split the module into focused submodules ([391d7e1](https://github.com/burntfern/slate/commit/391d7e17b0fc200f97e1c63d68b3cce3eb4ca1eb))
* **core:** split the core and config modules ([120fb09](https://github.com/burntfern/slate/commit/120fb094e84791faf176d7fe98adf201df74c1af))


### Documentation

* **readme:** restructure the README and refresh the gallery captures [skip ci] ([30c8835](https://github.com/burntfern/slate/commit/30c883505a3451b9bd1f5587e319b99195207642))
* **repo:** add the readme, contributing guide, and license ([3ef90ab](https://github.com/burntfern/slate/commit/3ef90abf5cef4d9e3279968d5a8cd4bc936b9c5e))
* **repo:** add the security policy and project metadata ([e57e514](https://github.com/burntfern/slate/commit/e57e514b98b336ed5f98e29f99bf83542af69adb))
