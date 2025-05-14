# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
### Changed
 - Use pedantic mode ([gh-24] [@rkuris])
### Deprecated
### Removed
### Fixed
### Security

[gh-24]: https://github.com/kellerkindt/onewire/pull/24

## [0.4.0] - 2025-05-12
### Added
### Changed
 - Upgrade to Rust 2018 edition, take pin by ownership, make search iter nicer ([1b1cd46], [@kellerkindt])
 - Remove duplicate select in reset_select_{write,read}_only ([90db8e5], [gh-16], [@asdfuser])
 - embedded-hal-1 support ([6b79ef0], [gh-18], [@andresv], [@simonborje])
 - Defmt Support ([58682f4], [gh-20], [@Feriixu])
 - Add embassy_rp2040 example with temperature sensor demo ([43f2e92], [gh-22], [@Feriixu])
 - Minor cleanup ([2b32deb], [gh-21], [@Feriixu])
 - Use a lookup table for faster CRC calculations ([1d32c44], [gh-23], [@rkuris])
### Deprecated
### Removed
### Fixed
### Security

[@kellerkindt]: https://github.com/kellerkindt
[@asdfuser]: https://github.com/asdfuser
[@andresv]: https://github.com/andresv
[@simonborje]: https://github.com/simonborje
[@Feriixu]: https://github.com/Feriixu
[@rkuris]: https://github.com/rkuris
[gh-16]: https://github.com/kellerkindt/onewire/pull/16
[gh-18]: https://github.com/kellerkindt/onewire/pull/18
[gh-20]: https://github.com/kellerkindt/onewire/pull/20
[gh-21]: https://github.com/kellerkindt/onewire/pull/21
[gh-22]: https://github.com/kellerkindt/onewire/pull/22
[gh-23]: https://github.com/kellerkindt/onewire/pull/23
[1b1cd46]: https://github.com/kellerkindt/onewire/commit/1b1cd46377ac40abd20e8843519678bd6a2b2cf3
[90db8e5]: https://github.com/kellerkindt/onewire/commit/90db8e5e86443be3c8afbb099b3a8d921128d043
[6b79ef0]: https://github.com/kellerkindt/onewire/commit/6b79ef00bd3871b4ee6052f8f30c4743c5597cfd
[58682f4]: https://github.com/kellerkindt/onewire/commit/58682f43c8cd85219e89f77ccefd21c516c28e0d
[43f2e92]: https://github.com/kellerkindt/onewire/commit/43f2e92245bce99830994d95faf6d3a894028c82
[2b32deb]: https://github.com/kellerkindt/onewire/commit/2b32deb9a1d88716d2abc7f475ae0394ddf80bc7
[1d32c44]: https://github.com/kellerkindt/onewire/commit/1d32c449ef15c25e0822a2486a6df269cf52c7f9
