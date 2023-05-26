# FOSSGraph

WIP: A knowledge graph for FOSS dependencies

## Supported resolution files

Populating dependency resolutions is the most complicated part of whole pipeline. This can be done ahead of time by the lockfile mechanism if the project uses a package manager.

FOSSGraph supports package managers that offically support the lockfile, otherwise it doesn't.

- [ ] NPM - `package-lock.json`
- [ ] PNPM - `pnpm-lock.yaml`
- [ ] Yarn (v1) - `yarn.lock`
- [ ] Yarn (Berry) - `yarn.lock`
- [ ] CocoaPods - `Podfile.lock`
- [ ] Swift Package Manager - `Package.resolved`
- [ ] Gradle - `gradle.lockfile`
- [ ] RubyGems - `Gemfile.lock`
- [ ] Cargo - `Cargo.lock`
- [ ] Go Modules - `go.sum`
- [ ] Poetry - `poetry.lock`
