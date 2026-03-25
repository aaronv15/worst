# Worst project manager/switcher

I have a lot of projects that I often switch between. Often in different languages and in different directories. And with my development environment being split amongst different shells, it can often be quite irritating to switch between projects. This project's purpose is to remedy that.

The goal is to be able to easily switch between projects with one command, and to open/navigate to the most recently accessed project with minimal effort.

## How it works

A argument is given to the program detailing which action to perform. Based on the given subcommand and a configuration file, a shell command is built and printed to stdout. This output string is captured by a wrapper shell script that runs this command, and then 'exec's the output if the program exited without errors.
Is this secure? Probably not, as it is reading arbitrary shell commands from a config file, and then executing those commands. But, it _may_ save you several seconds over the years, so I think it is probably worth the risk.
It might not be a bad idea to give root ownership of the config file and allow readonly access, but what what do I know, I'm not a security researcher.

## Setup

There is a script called worst.bash that is responsible for running the program. It has a variable named 'bin_path' which should be set to the path to the program binary. This shell script is what should be run. It needs to be sourced to be capable of changing directory in the current shell (as a subshell has no way to modify a parent's shell's environment), so an alias should be set up

It is also expected that there will be a file named 'config.toml' in a directory named 'worst-switcher' in $XDG_CONFIG_HOME
The path to the config file can be specified in the command line arguments.

The config.toml file has the following format:

```toml
# All keys / tables are optional, with default values being:
#   base_dir = '{std::env::var("HOME")}/Documents'
#   new = "mkdir '%{path}'"
#   go = "cd '%{path}'"
#   open = "nvim ."
# "%{path}" is one of (currently) four variables that will be substituted for a specific value.
# Those variables are:
# %{name}: Name of the project being switched to
# %{lang}: Language directory
# %{base_dir}: Base directory
# %{path}: Full path to project directory (= %{base_dir}/%{lang}/%{name})
#
# The base directory that all paths will be regarded as relative to. (Unless overidden)
base_dir = "/home/me/Documents"
# Command to run when 'go' is used
go = 'cd "%{path}"'
# The command to run when 'new' is used
new = 'mkdir "%{path}"'
# The command to run when 'open' is used
open = "nvim ."

[rust]
new = 'cargo new "%{path}"'

[python]
new = 'uv init "%{path}"'
go = 'cd "%{path}"; source ".venv/bin/activate"'

[pip.python]
# This is a user variable. It can be used as %{env} and will be replaced with the actual value.
# The variable can be of any valid toml type other than a Table, or a type that contains a table
# (This is currently basically useless, but if I eventually have the option to specify variables via the command line, they will be useful)
vars.env = "env"
# This overrides base_dir in the root table when the language is python and the version is pip
base_dir = "/home/me/Documents/old_python"
new = 'mkdir "%{path}"; python3 -m venv "%{path}/%{env}"'
go = 'cd "%{path}"; source "%{env}/bin/activate"'
```

## Usage:

There are currently six subcommands. The details and usage of each subcommand can be found by passing the help command to each subcommand, but the overview is:

- new - Creates a new project in a language
- go - Switches to a specified project directory
- open - Opens the project in (default) nvim
- go-new - Combines the new and go commands
- open-new - Combines the new and open commands
- setup - Generate config file and runner file
