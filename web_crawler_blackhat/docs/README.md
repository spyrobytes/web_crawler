# Web Crawler from the book "Black Hat Rust" by Sylvain Kerkour

[Repo](https://github.com/skerkour/black-hat-rust/tree/main/ch_05)

## Usage

```shell
$ cargo run -- spiders
$ cargo run -- run --spider cvedetails
```

## fmt

```shell
$ cargo fmt
```

## Install `chromedriver`

```shell
# Ubuntu
$ sudo apt install chromium-browser chromium-chromedriver

# Fedora
$ sudo dnf install chromium chromedriver
```


### Run chromedriver

```shell
$ chromedriver --port=4444 --disable-dev-shm-usage
```

## `chromedriver` Options

`chromedriver` is a standalone server that implements the WebDriver's wire 
protocol for `Chromium`. It is used to remotely control `Chromium`/`Chrome`
instances. WebDriver is an open source tool for automated testing of webapps 
across many browsers.

[WebDriver's wire protocol](https://www.w3.org/TR/webdriver/)

```shell
$ chromedriver --help
```
`--disable-dev-shm-usage`: Disable the use of `/dev/shm` for shared memory. Do 
not use `/dev/shm` (add this switch if seeing errors related to shared memory).

`--port`: Port to listen on.

`--verbose`: Log verbosely.
