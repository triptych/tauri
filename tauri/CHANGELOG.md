# Changelog

## [0.7.4]

-   Ignoring non UTF-8 characters on the loopback command output.
    -   [f340b29](https://www.github.com/tauri-apps/tauri/commit/f340b2914dc9c3a94ca8606f4663964fa87b95ea) fix(tauri) addition to the previous commit on 2020-07-10

## [0.7.3]

-   Properly run the loopback command on Windows.
-   Properly ignore the ${distDir}/index.html asset from the asset embbeding. Previously every asset with name matching /(.+)index.html$/g were ignored.

## [0.7.2]

Bumped due to dependency.

## [0.7.1]

-   Fixes the assets embedding into the binary.

## [0.7.0]

-   The execute_promise and execute_promise_sync helpers now accepts any tauri::Result<T> where T: impl Serialize.
    This means that you do not need to serialize your response manually or deal with String quotes anymore.
    As part of this refactor, the event::emit function also supports impl Serialize instead of String.

## [0.6.2]

-   Fixes the Windows build with the latest Windows SDK.

## [0.6.1] - (Not Published)

## [0.6.0]

-   Adds a command line interface option to tauri apps, configurable under tauri.conf.json > tauri > cli.
-   Fixes no-server mode not running on another machine due to fs::read_to_string usage instead of the include_str macro.
    Build no longer fails when compiling without environment variables, now the app will show an error.
-   Adds desktop notifications API.
-   Properly reflect tauri.conf.json changes on app when running tauri dev.
