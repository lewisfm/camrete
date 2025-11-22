using Camrete.Core;

Console.WriteLine("Hello, World!");

var manager = new RepoManager("../../development.db");
var repoDB = manager.Database();

foreach (var name in repoDB.AllRepos(createDefault: true))
{
    Console.WriteLine($"Repo name: {name}");
}
