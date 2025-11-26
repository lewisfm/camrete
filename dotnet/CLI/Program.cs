using Camrete.Core;
using System.CommandLine;

RootCommand rootCommand = new("Sample .NET app for Camrete");
rootCommand.Subcommands.Add(new ShowModule());

return rootCommand.Parse(args).Invoke();
