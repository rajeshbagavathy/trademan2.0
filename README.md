# Tradman

Paper multi-leg options desk. A Rust engine places and tracks paper orders. A React page is the monitor: create a strategy, activate it, and watch it queue until its entry time.

The architecture, the product decisions, and what is deliberately not enforced yet are in [docs/DESIGN.md](docs/DESIGN.md). Cursor loads the short form from `.cursor/rules/trademan.mdc`. Read the design before adding a feature.

This build does not send live broker orders and does not need API keys.

The same commands run on macOS and Windows. Entry times use the computer's local timezone on both.

## What you need

Install these once. Use the latest stable release of each.

| Tool | macOS | Windows |
| --- | --- | --- |
| Git | Xcode Command Line Tools, or Git from [git-scm.com](https://git-scm.com) | [git-scm.com](https://git-scm.com) |
| Rust | [rustup](https://rustup.rs) | [rustup](https://rustup.rs), then the MSVC C++ linker below |
| Node.js 20 or newer | [nodejs.org](https://nodejs.org) | [nodejs.org](https://nodejs.org) |

Check the tools:

```bash
git --version
rustc --version
cargo --version
node --version
npm --version
```

### macOS

1. Install the compiler tools. In Terminal:

   ```bash
   xcode-select --install
   ```

2. Install Rust from https://rustup.rs. The default host is `aarch64-apple-darwin` on Apple silicon and `x86_64-apple-darwin` on Intel. Accept the defaults.

3. Install Node.js 20 or newer from https://nodejs.org. The LTS installer is enough.

4. Open a new Terminal window so `cargo` and `node` are on `PATH`.

### Windows

1. Install Git from https://git-scm.com. The default options are fine.

2. Install Rust from https://rustup.rs. Choose the default toolchain, `stable-x86_64-pc-windows-msvc`.

3. Rust on Windows links with Microsoft's C++ tools. Install **Build Tools for Visual Studio 2022** and select the workload **Desktop development with C++**. A full Visual Studio install with that workload also works. Without `link.exe`, `cargo build` stops at the link step.

4. Install Node.js 20 or newer from https://nodejs.org. The LTS installer is enough.

5. Open a new PowerShell window so `cargo` and `node` are on `PATH`.

The desk listens on port **8090**, not 8080. On some Windows machines 8080 is reserved and the API cannot bind there. macOS uses the same 8090 and 5173 ports.

## Get the code

```bash
git clone https://github.com/rajeshbagavathy/trademan2.0.git
cd trademan2.0
```

On Windows, PowerShell uses the same `git clone` line. `cd` works in both shells.

## Run it

Use two terminals. Both stay open while you use the desk.

**Terminal 1, engine.** From the repository root:

```bash
cargo run -p api
```

Wait until it says it is running `api`. The paper API is then at http://127.0.0.1:8090. The first run downloads crates and can take several minutes.

**Terminal 2, page.** From the repository root:

```bash
cd ui
npm install
npm run dev -- --host 127.0.0.1 --port 5173
```

`npm install` is only required the first time, or after `ui/package-lock.json` changes.

Open http://127.0.0.1:5173.

- **Create** builds a strategy and saves it.
- **Algo trade** lists saved strategies at the bottom. **Activate** moves one to the top board.
- If the entry time is still ahead, the card counts down and no paper orders are sent. At the entry time the engine enters and the timer shows elapsed time.
- If the entry time has already passed today, it enters immediately and the timer runs.

The seeded Nifty book is already live so you can see combined P&L. Bank Nifty stays saved until you activate it.

## Check the build

From the repository root:

```bash
cargo test --workspace
```

From `ui`:

```bash
npx tsc --noEmit
```

## Notes

- Paper only. Broker names on the activate dialog select a paper route. Do not put broker passwords, tokens, or API keys in this repository.
- Prices inside the engine are integer paise. The page shows rupees.
- Stopping is closing both terminals, or pressing Ctrl+C in each one.
