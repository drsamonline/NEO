# Privacy

Version: 1.0
Effective: 09 November 2026
Owner: Dr Sohil Momin — sohilmomin2000@gmail.com
Changes: Initial version.

## What this app is

NOVA — a statically typed, compiled systems programming language whose compiler lowers to C, JavaScript, Rust and WebAssembly.

It runs on your own device. There is no account, no server and no sign-in.

## What it collects

**Nothing.** No analytics, no telemetry, no crash reporting, no fingerprinting, no advertising identifier, no
email capture.

**What that means for each part**, because this repository ships more than one program:

| Part | Reaches the network? |
|---|---|
| `nova-compiler`, `nova-lsp`, `nova-dbg` | **No.** No HTTP client, no socket code — checked: no `reqwest`/`hyper`/`ureq` dependency and no `TcpStream`, `UdpSocket` or `std::net` use in their sources. |
| the landing page (`docs/`) | Only to load its own pages; a service worker caches them on your device so it works offline. |
| `nova-pkg` | **Only for a `git` dependency**, and only by running `git clone` with your own git configuration and credentials. It contains no HTTP client of its own, and a `path` dependency touches nothing. The host contacted is the one written in your `Nova.toml`, so you choose it. |

No request carries anything you type or store: `nova-pkg` sends the URL in your manifest and nothing else, and the compiler sends nothing at all.

## What is stored on your device

This app remembers things between visits. They are kept on this device — nothing more. Clearing the app's stored data erases them.

## Links that leave the app

Documentation, the project index and the support page contain ordinary links. Opening one takes you to that
site, with its own privacy policy; the app loads nothing from it.

## Erasing

Clear the app's stored data, or delete the app itself. Nothing is held anywhere else, so there is no other
copy to ask us to remove.

## Changes

If a future version ever adds a request that carries your data, this file changes in the same commit.

Contact: Dr Sohil Momin — sohilmomin2000@gmail.com
