# Changelog

All notable changes to this project will be documented in this file. See [standard-version](https://github.com/conventional-changelog/standard-version) for commit guidelines.

## [1.12.0](https://github.com/mseele/sve-backend/compare/v1.11.1...v1.12.0) (2021-12-07)


### Features

* make booking button text configurable ([168b21d](https://github.com/mseele/sve-backend/commit/168b21da76ab4be6b1dd72adf0a3710c468cc69f))

### [1.11.1](https://github.com/mseele/sve-backend/compare/v1.11.0...v1.11.1) (2021-08-26)


### Bug Fixes

* query parameter name ([d51b486](https://github.com/mseele/sve-backend/commit/d51b486ebec0feb2e6c5f62c45e7f4e281b5a45e))

## [1.11.0](https://github.com/mseele/sve-backend/compare/v1.10.0...v1.11.0) (2021-08-26)


### Features

* add new all param for events api ([64b635e](https://github.com/mseele/sve-backend/commit/64b635e76fa6c097d51cd7c5c11f050335e1c4e9))


### Bug Fixes

* remove caching ([2549e5b](https://github.com/mseele/sve-backend/commit/2549e5ba5630ae0b55f4b426e9788acb67ce93a4))
* remove race-condition when multiple bookings happen in parallel ([d1cc964](https://github.com/mseele/sve-backend/commit/d1cc964fddef2301211616a4c13e9df892172e3f))
* switch to DEBUG filter ([7cf4db2](https://github.com/mseele/sve-backend/commit/7cf4db2ea3f2cea0b5b98fb08b5c0559d8c8d6dd))

## [1.10.0](https://github.com/mseele/sve-backend/compare/v1.9.2...v1.10.0) (2021-01-09)


### Features

* **tasks:** add check email connectivity task ([ddd58b3](https://github.com/mseele/sve-backend/commit/ddd58b3b6083ecd20419aa9cd20b0e2008aae626))
* **tasks:** add renew calendar watch task ([48234a1](https://github.com/mseele/sve-backend/commit/48234a1bafd335d9c3d71e099a270609f237ff11))

### [1.9.2](https://github.com/mseele/sve-backend/compare/v1.9.1...v1.9.2) (2020-11-08)


### Bug Fixes

* **cors:** do not use wildcard to work on older browsers ([b6a2ba3](https://github.com/mseele/sve-backend/commit/b6a2ba3bacf7147441031e54095d6b8ae87fc693))
* **store:** fix possible NPE when using customDate ([831ad05](https://github.com/mseele/sve-backend/commit/831ad05343050b6f38901a29610e1e80129f73a9))

### [1.9.1](https://github.com/mseele/sve-backend/compare/v1.9.0...v1.9.1) (2020-08-21)


### Bug Fixes

* **email:** fix bcc recipients bug ([335ceca](https://github.com/mseele/sve-backend/commit/335ceca3990008c0488115d8bd73249ef72064c7))

## [1.9.0](https://github.com/mseele/sve-backend/compare/v1.8.0...v1.9.0) (2020-08-13)


### Features

* **calendar:** trigger re-deploy on calendar change ([ff5dbe1](https://github.com/mseele/sve-backend/commit/ff5dbe1634622c8b8c4de3a44f005cd40a67a259))
* **email:** send multiple emails via 1 smtp session ([9e22c80](https://github.com/mseele/sve-backend/commit/9e22c808745355b65e0c11ba301fd96d51529f5f)), closes [#31](https://github.com/mseele/sve-backend/issues/31)

## [1.8.0](https://github.com/mseele/sve-backend/compare/v1.7.0...v1.8.0) (2020-08-11)


### Features

* **calendar:** add html link to appointment ([5013ed4](https://github.com/mseele/sve-backend/commit/5013ed4f6d9ed9397a1037f99319ea850212d9e9)), closes [#34](https://github.com/mseele/sve-backend/issues/34)
* **calendar:** add notifications hook ([8e2ede5](https://github.com/mseele/sve-backend/commit/8e2ede568d207a8292b3c7a06bdf63063fda6996))
* **calendar:** add watch/stop calendar api calls ([0fc1634](https://github.com/mseele/sve-backend/commit/0fc16343764ae8c9b06c1939a24f15479d431e52))


### Bug Fixes

* **api:** add id and sortIndex to appointment ([642e4a2](https://github.com/mseele/sve-backend/commit/642e4a20bea8b0996581dfc96010840cf512d49a))

## [1.7.0](https://github.com/mseele/sve-backend/compare/v1.6.0...v1.7.0) (2020-07-27)


### Features

* **api:** implement calendar access ([7dca583](https://github.com/mseele/sve-backend/commit/7dca583a2cf955103a0e20681f56f948a7a02419))

## [1.6.0](https://github.com/mseele/sve-backend/compare/v1.5.1...v1.6.0) (2020-07-27)


### Features

* **events:** add refresh post request ([32bd4a1](https://github.com/mseele/sve-backend/commit/32bd4a1790da3021d8752d6318dbf2ed46b00a66))
* **prebooking:** identify duplicate link clicks ([fca7e5d](https://github.com/mseele/sve-backend/commit/fca7e5d44cb5bad38cf22bea24f898663aeda8a0))


### Bug Fixes

* **prebooking:** fix wrong if clause when checking prebooking ([6bc83d9](https://github.com/mseele/sve-backend/commit/6bc83d9c93fab7221626248df421f6be6d717cd4))
* **prebooking:** invalidate prebooking for non-beta events ([64ff51b](https://github.com/mseele/sve-backend/commit/64ff51b34ccf9e75986e79920a192220a3db2460))
* **prebooking:** log warn message if booking link is invalid ([4347c6b](https://github.com/mseele/sve-backend/commit/4347c6b567edcf66c8273bbdf3de240c6151a9e4))

### [1.5.1](https://github.com/mseele/sve-backend/compare/v1.5.0...v1.5.1) (2020-06-26)


### Bug Fixes

* improve error logging ([0669a55](https://github.com/mseele/sve-backend/commit/0669a55c983bdc4f00e8290e7406c17ef69d4443))

## [1.5.0](https://github.com/mseele/sve-backend/compare/v1.4.2...v1.5.0) (2020-06-26)


### Features

* **events:** implement pre-booking support ([c2a8618](https://github.com/mseele/sve-backend/commit/c2a8618900f7168d9dccf32ef121e5126546fbc1))
* new tools web ui for the backend ([0c73225](https://github.com/mseele/sve-backend/commit/0c7322584a694c67c3c59c1658659e42921e753a))

### [1.4.2](https://github.com/mseele/sve-backend/compare/v1.4.0...v1.4.1) (2020-06-09)
