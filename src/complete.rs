//! Shell completion: the scripts, and the candidate feeds they call back into.
//!
//! Two traps shaped this, both found on a real machine rather than reasoned out:
//!
//! - **zsh may have no completion system loaded at all.** A `.zshrc` that never
//!   calls `compinit` leaves `compdef` undefined, and a script that just calls
//!   `compdef` fails silently — Tab does nothing and completion looks broken.
//!   The emitted script bootstraps `compinit` itself.
//! - **bash truncates the line if a candidate does not start with the typed
//!   word.** readline replaces the word with the candidates' longest common
//!   prefix, so offering `bitcoin` for a typed `btc` *shortens* what you typed.
//!   Every candidate we emit therefore starts with the prefix as typed; see
//!   [`crate::coins::candidates`].

use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::coins;
use crate::config::{Config, Range};
use crate::data::COMMON_CURRENCIES;

const ZSH: &str = r#"# coins zsh completion. Install with:
#   coins completions install
# or by hand:
#   echo 'eval "$(coins completions zsh)"' >> ~/.zshrc
#
# Bootstraps zsh's completion system if the shell has not already done so. A bare
# .zshrc has no compinit, which leaves `compdef` undefined and Tab doing nothing
# anywhere — not just for coins. `-i` skips insecure completion files rather than
# trusting them, so this stays quiet without lowering the bar.
if ! whence compdef > /dev/null 2>&1; then
  autoload -Uz compinit && compinit -i
fi

_coins() {
  local -a cmds items nocase
  # Tickers and coin ids are lower case, but nobody should have to remember that
  # at a Tab; every filter in the tool folds case, so completion does too.
  nocase=(-M 'm:{a-zA-Z}={A-Za-z}')
  cmds=(
    'plot:Draw the full-size plots'
    'balance:What each address holds, and its value'
    'add:Track a coin, or an address'
    'rm:Stop tracking a coin or address'
    'config:Print the config path, or edit it'
    'completions:Shell completion scripts'
  )
  local cur=${words[CURRENT]} prev=${words[CURRENT-1]}

  # `--flag=value` reaches us as a single word, so strip the prefix and complete
  # the value. `compset -P` keeps the matched part on the line.
  case $cur in
    --*=*)
      local flag=${cur%%=*}
      compset -P "${flag}="
      prev=$flag
      cur=
      ;;
  esac

  case $prev in
    -c|--currency)
      items=(${(f)"$(coins completions currencies ${(Q)cur} 2>/dev/null)"})
      (( ${#items} )) && _describe -t currencies 'currency' items $nocase
      return
      ;;
    -r|--range)
      items=(${(f)"$(coins completions ranges 2>/dev/null)"})
      (( ${#items} )) && _describe -t ranges 'timeframe' items $nocase
      return
      ;;
  esac

  # A word being typed as a flag. Without this, `--ref<TAB>` does nothing at all,
  # which reads as broken completion rather than as "there is nothing there".
  if [[ $cur == -* ]]; then
    # `--label` belongs to `add` alone, so it is offered only there.
    local -a extra
    [[ $words[2] == add ]] && extra=('-l[name this address]' '--label[name this address]')
    _values $nocase 'option' $extra \
      '-c[quote in this currency]' '--currency[quote in this currency]' \
      '-r[timeframe: 1d 1w 1m 3m 6m 1y all]' '--range[timeframe: 1d 1w 1m 3m 6m 1y all]' \
      '--no-plot[leave the plots out]' '--refresh[ignore the cache and fetch now]' \
      '-h[help]' '--help[help]' '-V[version]' '--version[version]'
    return
  fi

  # The first word is either a subcommand or a coin, since `coins btc` works.
  if (( CURRENT == 2 )); then
    _describe -t commands 'coins command' cmds $nocase
    items=(${(f)"$(coins completions tracked ${(Q)cur} 2>/dev/null)"})
    (( ${#items} )) && _describe -t coins 'tracked coin' items $nocase
    return
  fi

  case ${words[2]} in
    add)
      items=(${(f)"$(coins completions coins ${(Q)cur} 2>/dev/null)"})
      if (( ${#items} )); then
        _describe -t coins 'coin' items $nocase
      else
        # `-r` because a bare `_message` is swallowed here and never shown.
        _message -r 'no popular coin starts with that — any CoinGecko id also works'
      fi
      ;;
    rm)
      items=(${(f)"$(coins completions removable ${(Q)cur} 2>/dev/null)"})
      (( ${#items} )) && _describe -t tracked 'tracked' items $nocase
      ;;
    plot)
      # Coins only: `coins plot` does not take an address.
      items=(${(f)"$(coins completions tracked ${(Q)cur} 2>/dev/null)"})
      (( ${#items} )) && _describe -t coins 'tracked coin' items $nocase
      ;;
    completions)
      _values $nocase 'target' 'install[wire it into your shell]' 'zsh' 'bash'
      ;;
    config)
      _values $nocase 'flag' '--edit[open in $EDITOR]'
      ;;
  esac
}

compdef _coins coins
"#;

const BASH: &str = r#"# coins bash completion. Install with:
#   coins completions install
# or by hand:
#   echo 'eval "$(coins completions bash)"' >> ~/.bashrc

_coins_offer() {
  # $1 newline-separated candidates, $2 the word being completed.
  #
  # Every entry put in COMPREPLY must begin with $2 *exactly*. readline works out
  # the longest common prefix of the matches case-sensitively and replaces the
  # typed word with it, so a single candidate that does not share the prefix
  # shortens the line instead of extending it. `coins completions` guarantees
  # this by offering matching ids and matching tickers, never an id for a ticker.
  local IFS=$'\n'
  COMPREPLY=($(compgen -W "$1" -- "$2"))
  if [ ${#COMPREPLY[@]} -gt 0 ]; then return; fi
  # Nothing matched as typed; fold case, since every filter in the tool does.
  local lower
  lower=$(printf '%s' "$2" | tr '[:upper:]' '[:lower:]')
  COMPREPLY=($(compgen -W "$1" -- "$lower"))
}

_coins() {
  local cur prev cmd
  cur=${COMP_WORDS[COMP_CWORD]}
  prev=${COMP_WORDS[COMP_CWORD-1]}
  cmd=${COMP_WORDS[1]}

  case $prev in
    -c|--currency)
      _coins_offer "$(coins completions currencies "$cur" --plain 2>/dev/null)" "$cur"
      return
      ;;
    -r|--range)
      _coins_offer "$(coins completions ranges --plain 2>/dev/null)" "$cur"
      return
      ;;
  esac

  if [[ $cur == -* ]]; then
    # `--label` belongs to `add` alone, so it is offered only there.
    local label=""
    [ "${COMP_WORDS[1]}" = "add" ] && label="-l
--label"
    _coins_offer "$label
-c
--currency
-r
--range
--no-plot
--refresh
--help
--version" "$cur"
    return
  fi

  if [ "$COMP_CWORD" -eq 1 ]; then
    _coins_offer "plot
balance
add
rm
config
completions
$(coins completions tracked "$cur" --plain 2>/dev/null)" "$cur"
    return
  fi

  case $cmd in
    add)
      _coins_offer "$(coins completions coins "$cur" --plain 2>/dev/null)" "$cur"
      ;;
    rm)
      _coins_offer "$(coins completions removable "$cur" --plain 2>/dev/null)" "$cur"
      ;;
    plot)
      # Coins only: `coins plot` does not take an address.
      _coins_offer "$(coins completions tracked "$cur" --plain 2>/dev/null)" "$cur"
      ;;
    completions)
      _coins_offer "install
zsh
bash" "$cur"
      ;;
    config)
      _coins_offer "--edit" "$cur"
      ;;
  esac
}

complete -F _coins coins
"#;

/// `coins completions <what> [PREFIX] [--plain]`.
pub fn run(what: &str, prefix: Option<&str>, plain: bool) -> Result<()> {
    let prefix = prefix.unwrap_or("");
    match what {
        "zsh" => print!("{ZSH}"),
        "bash" => print!("{BASH}"),
        "install" => return install(if prefix.is_empty() { None } else { Some(prefix) }),
        "coins" => emit(coins::candidates(prefix), plain),
        "tracked" => emit(tracked(prefix, false), plain),
        "removable" => emit(tracked(prefix, true), plain),
        "currencies" => emit(currencies(prefix), plain),
        "ranges" => emit(ranges(), plain),
        other => bail!(
            "unknown completion target {other:?} — use install, zsh, bash, coins, tracked, removable, currencies or ranges"
        ),
    }
    Ok(())
}

/// zsh's `_describe` wants `candidate:description`; bash wants the bare word.
fn emit(items: Vec<(impl AsRef<str>, impl AsRef<str>)>, plain: bool) {
    let mut out = String::new();
    for (candidate, description) in items {
        let candidate = candidate.as_ref();
        if plain || description.as_ref().is_empty() {
            out.push_str(candidate);
        } else {
            // A description can't contain the separator zsh splits on.
            let clean = description.as_ref().replace(':', " ");
            out.push_str(&format!("{candidate}:{clean}"));
        }
        out.push('\n');
    }
    print!("{out}");
}

/// What the config currently tracks. Addresses are included only for `rm`,
/// which accepts them: `coins plot` and a bare coin argument take a coin, so
/// offering an address there completes to something that cannot be run.
fn tracked(prefix: &str, with_addresses: bool) -> Vec<(String, String)> {
    let p = prefix.trim().to_ascii_lowercase();
    let Ok((cfg, _)) = Config::load() else { return Vec::new() };
    let mut out: Vec<(String, String)> = Vec::new();
    for id in &cfg.coins {
        if id.starts_with(&p) {
            let name = coins::resolve(id).map(|c| c.2.to_string()).unwrap_or_default();
            out.push((id.clone(), name));
        } else if let Some(c) = coins::resolve(id) {
            // The id does not match but its ticker does; offer the ticker,
            // which `coins rm` resolves against the tracked list just as well.
            if c.1.starts_with(&p) {
                out.push((c.1.to_string(), c.2.to_string()));
            }
        }
    }
    if with_addresses {
        for w in &cfg.wallets {
            if w.address.to_ascii_lowercase().starts_with(&p) {
                out.push((
                    w.address.clone(),
                    w.label.clone().unwrap_or_else(|| "wallet".into()),
                ));
            }
        }
    }
    out
}

fn currencies(prefix: &str) -> Vec<(String, String)> {
    let p = prefix.trim().to_ascii_lowercase();
    COMMON_CURRENCIES
        .iter()
        .filter(|c| c.starts_with(&p))
        .map(|c| (c.to_string(), String::new()))
        .collect()
}

fn ranges() -> Vec<(String, String)> {
    ["1d", "1w", "1m", "3m", "6m", "1y", "all"]
        .iter()
        .map(|r| {
            let label = Range::parse(r).map(|r| r.label().to_string()).unwrap_or_default();
            (r.to_string(), label)
        })
        .collect()
}

/// Appends the one line that turns completion on, idempotently.
fn install(shell: Option<&str>) -> Result<()> {
    let shell = match shell {
        Some(s) => s.trim().to_string(),
        None => std::env::var("SHELL")
            .ok()
            .and_then(|s| s.rsplit('/').next().map(|s| s.to_string()))
            .unwrap_or_else(|| "zsh".into()),
    };
    let (rc, line) = match shell.as_str() {
        "zsh" => (".zshrc", r#"eval "$(coins completions zsh)""#),
        "bash" => (".bashrc", r#"eval "$(coins completions bash)""#),
        other => bail!(
            "no completion script for {other:?} — coins supports zsh and bash\n\
             run `coins completions install zsh` to pick one explicitly"
        ),
    };
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("$HOME is not set"))?;
    let path = home.join(rc);

    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.contains("coins completions") {
        println!("completion is already set up in {}", path.display());
        return Ok(());
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("opening {}", path.display()))?;
    // A leading newline in case the file does not end in one.
    writeln!(file, "\n# coins — tab completion for coins, currencies and ranges\n{line}")
        .with_context(|| format!("writing {}", path.display()))?;
    println!("added completion to {}", path.display());
    println!("open a new shell (or run `{line}`) to use it");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zsh_script_bootstraps_compinit() {
        // Without this, a .zshrc that never calls compinit gets silent failure.
        assert!(ZSH.contains("whence compdef"));
        assert!(ZSH.contains("compinit -i"));
        let name = env!("CARGO_PKG_NAME");
        assert!(ZSH.trim_end().ends_with(&format!("compdef _{name} {name}")));
    }

    #[test]
    fn bash_script_registers_itself() {
        let name = env!("CARGO_PKG_NAME");
        assert!(BASH.trim_end().ends_with(&format!("complete -F _{name} {name}")));
        assert!(BASH.contains("compgen -W"));
    }

    #[test]
    fn ranges_are_described() {
        let r = ranges();
        assert_eq!(r.len(), 7);
        assert!(r.iter().any(|(k, v)| k == "1w" && v == "7 days"));
    }

    #[test]
    fn currencies_filter_by_prefix() {
        let c = currencies("eu");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].0, "eur");
        assert!(currencies("").len() > 20);
    }
}
