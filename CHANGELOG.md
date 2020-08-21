# Changelog

All notable changes to this project will be documented in this file. See [standard-version](https://github.com/conventional-changelog/standard-version) for commit guidelines.

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
