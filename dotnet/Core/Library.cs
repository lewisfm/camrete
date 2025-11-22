using System.Reflection;
using System.Runtime.InteropServices;
using Camrete.Core;

public static class CamreteCore
{
    public static ulong Add(ulong left, ulong right)
    {
        return CamreteMethods.Add(left, right);
    }
}
