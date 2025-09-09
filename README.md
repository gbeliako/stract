<p align="center">
  <img src="assets/big-logo.svg" height="120" />
  <br />
  <br />
  <a target="_blank" href="https://docs.stract.com/">
    <img src="https://img.shields.io/badge/api-docs-0b7bff?labelColor=white" style="max-width: 100%;">
  </a>
  <a target="_blank" href="https://stract.com/webmasters">
    <img src="https://img.shields.io/badge/webmaster-info-0b7bff?labelColor=white" style="max-width: 100%;">
  </a>
</p>
<br />

Stract is an open source web search engine hosted at [stract.com](https://stract.com/) targeted towards tinkerers and developers.

<br />
<p align="center">
  <img src="assets/screenshot.png" width="80%" />
</p>
<br />
<br />

# 💡 Features

- Keyword search that respects your search query.
- Fully independent search index [with our own crawler](https://stract.com/webmasters).
- Advanced query syntax (`site:`, `intitle:` etc.).
- DDG-style [!bang syntax](https://duckduckgo.com/bang)
- Wikipedia and stackoverflow sidebar
- De-rank websites with third-party trackers
- Use [optics](https://github.com/StractOrg/sample-optics/blob/main/quickstart.optic) to almost endlessly customize your search results.
  - Limit your searches to blogs, indieweb, educational content etc.
  - Customize how signals are combined during search for the final search result
- Prioritize links (centrality) from the sites you trust.
- Explore the web and find sites similar to the ones you like.
- And much more!

# 👩‍💻 Setup

We recommend everyone to use the hosted version at [stract.com](https://stract.com/), but you can also follow the steps outlined in [CONTRIBUTING.md](CONTRIBUTING.md) to setup the engine locally.

# Development

## Setup

- Install rust by following the steps outlined [here](https://www.rust-lang.org/tools/install). (or just sudo apt install cargo pkg-config wasm)
- Install clang and npm. (sudo apt install clang npm)
- Install liburing. If you're using a Debian based Linux, you can install it by running `sudo apt install liburing-dev`.
- Install [just](https://github.com/casey/just) by running `cargo install just`. This allows you to run the scripts in the `justfile` file. A justfile is basically a simple Makefile.
- Install [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) by running `cargo install wasm-pack`. This will allow for generation and packaging of client side WebAssembly code.
- Run the command `just configure` which should automatically configures the rest of your dev environment. The script creates a python virtual environment, installs relevant dependencies and creates a small local index which you can use for development. Run `just configure --ml` instead if you also want to setup the ML models. However, you may need to input S3 credentials to download some ML model from upstream S3 bucket.
- Install [cargo-watch](https://github.com/watchexec/cargo-watch) by running `cargo install cargo-watch`.
- (Optional) Install [abeye](https://github.com/oeb25/abeye) by running `cargo install --git https://github.com/oeb25/abeye --locked`. This is used for generating the API client used by the frontend by running `just openapi`. (that didn't work for me...)

After the non optional steps you can now run `cargo test` and should see all tests passing. If you have installed `cargo-watch`, you should be able to run `just dev` to start the search server and launch the frontend at `0.0.0.0:8000`.

- in case of an error about optics, need to make sure that crates/optics/testcases/samples/ contains the necessary ones. might have to download manually.
- need to modify frontend/.env to point at the correct server ip or hostname
- Stract can be run with "just dev" or "just dev --release".
- Recommend to update npm packages by:
    cd frontend/
    npx update-browserslist-db@latest
    npm audit fix (or npm audit fix --force)
- Can also build the frontend via 'npm run build'. If so, then can modify the justfile to run 'npm run preview' instead of 'npm run dev'.

Running 'just dev' starts the frontend, and then runs a python script in scripts/run_dev.py, which just forks some stract processes.
The config files are in configs/
To build the index you would need to perform the following main steps

    1. (optionally) run the crawler to crawl some pages and save them in .warc files. The current crawler architecture requires a crawl plan to be built before the crawl can be executed. Commoncrawl distributes a giant dataset of these files, so you can actually totally skip having to run Stracts own crawler, which makes it a lot easier to get started.
      This can be done via: (./target/release/)stract crawler plan ./configs/crawler/planner.toml
      then, to crawl need to start the coordinator, router an a number of workers. 
    2. build the webgraph using the stract webgraph create command. The config file you want to look at here is located at configs/webgraph/create.toml.
    3. calculate the harmonic centrality for each page/host using stract centrality.
    4. build the index using stract indexer search. The config file configs/indexer/create.toml should help get you started.

This should create an index which you can run and execute searches against. Unfortunately I don't have a neat overview of the available fields in each config file, but all of them are defined in crates/core/src/config/mod.rs.

Actually, I slightly modified Stract to have the option to use WET files instead of WARC files to build the index. This is because Common Crawl has over 80TB of WARC files, which makes it impractical to build an index without a lot of disks, or without an expensive S3 instance. Luckily, CommonCrawl also provides WET files, which just contain the plain-text of the websites.
However, without the html links in the WARC files, we can't build the webgraph to calculate host centrality values. But luckily, CommonCrawl provides pre-computed host centrality values (and also the webgraph, tho it's in a different format).

in indexer/create.toml:
use_wet_files = true/false
wet_harmonic_centrality_path = "./data/file.gz"

and in warc_source or in wet_source, provide the names of the downloaded WET files.

# ‍💼 License

Stract is offered under the terms defined under the [LICENSE.md](LICENSE.md) file unless otherwise specified in the relevant subdirectory.

# 📬 Contact

You can contact us at [hello@stract.com](mailto:hello@stract.com) or open an [issue](https://github.com/StractOrg/stract/issues)/[discussion](https://github.com/StractOrg/stract/discussions) here on GitHub.

# 🏆 Thank you!

We truly stand on the shoulders of giants and this project would not have been even remotely feasible without them. An especially huge thank you to

- The authors and contributors of Tantivy for providing the inverted index library on which Stract is built.
- The commoncrawl organization for crawling the web and making the dataset readily available. Even though we have our own crawler now, commoncrawl has been a huge help in the early stages of development.

# 💰 Funding

This project was previously funded through [NGI0 Entrust](https://nlnet.nl/entrust), a fund established by [NLnet](https://nlnet.nl) with financial support from the European Commission's [Next Generation Internet](https://ngi.eu) program. Learn more at the [NLnet project page](https://nlnet.nl/project/Stract).

<div>
  <a href="https://nlnet.nl"><img align=center src="assets/nlnet/banner.png" alt="NLnet foundation logo" width="20%" /></a>
  &nbsp;
  &nbsp;
  <a href="https://nlnet.nl/entrust"><img align=center src="assets/nlnet/NGI0_tag.svg" alt="NGI Zero Logo" width="20%"/></a>
</div>
