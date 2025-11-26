using System.CommandLine;
using Camrete.Core;

class ShowModule : Command
{
    private static readonly Argument<string> modSlugArg = new("module-slug")
    {
        Description = "The textual identifier of the module.",
        Arity = ArgumentArity.ExactlyOne,
    };

    public ShowModule() : base("show", "View a modules's information")
    {
        Arguments.Add(modSlugArg);
        SetAction(parseResult => Show(parseResult.GetValue(modSlugArg)!));
    }

    private static void Show(string identifier)
    {
        var manager = new RepoManager("../../development.db");
        var repoDB = manager.Database();

        var module = repoDB.ModuleBySlug(identifier);
        if (module == null)
        {
            Console.Error.WriteLine($"No such module: {identifier}");
            Environment.Exit(1);
        }

        var releases = repoDB.ReleasesWithParent(module.id);
        var first = releases.First();
        if (first == null)
        {
            Console.Error.WriteLine($"No releases for module: {identifier}");
            Environment.Exit(1);
        }

        var associatedData = repoDB.AssociatedReleaseData(first.id);

        var authors = string.Join(", ", associatedData.authors);
        var tags = string.Join("", associatedData.tags.Select(t => $" #{t}"));
        var licenses = string.Join(" and ", associatedData.licenses);

        Console.Write($"{first.displayName}, {first.version}{tags}");
        if (first.releaseStatus != ReleaseStatus.Stable)
        {
            Console.Write($" ({first.releaseStatus})");
        }
        Console.WriteLine();
        Console.WriteLine();

        Console.WriteLine($"{first.summary}");
        Console.WriteLine();

        if (first.description != null)
        {
            Console.WriteLine($"\n{first.description}");
            Console.WriteLine();
        }

        var resources = first.metadata.resources;
        if (resources.homepage != null)
        {
            Console.WriteLine($"{resources.homepage}");
        }

        Console.WriteLine($"Authors: {authors}");
        Console.WriteLine($"License: {licenses}");

        if (resources.bugtracker != null)
        {
            Console.WriteLine($"Bug tracker: {resources.bugtracker}");
        }
        if (resources.repository != null)
        {
            Console.WriteLine($"Repository: {resources.repository}");
        }
        if (resources.spacedock != null)
        {
            Console.WriteLine($"Spacedock: {resources.spacedock}");
        }


        if (first.releaseDate != null)
        {
            Console.WriteLine($"Release Date: {first.releaseDate}");
        }

        if (releases.Length > 1)
        {
            var others = string.Join(", ", releases.AsEnumerable()
                .Skip(1)
                .Select(release => release.version));
            Console.WriteLine($"Other versions: {others}");
        }
        Console.WriteLine();

        Console.WriteLine("Relationships:");
        var relationships = repoDB.RelationshipsForRelease(first.id);
        foreach (FullRelationship rel in relationships)
        {
            Console.WriteLine($"- {rel.description.targetName} ({rel.group.relType})");
        }
        Console.WriteLine();
    }
}
