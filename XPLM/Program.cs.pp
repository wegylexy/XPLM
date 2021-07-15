using FlyByWireless.XPLM;
using System.Runtime.InteropServices;
using System.Text;

namespace $RootNamespace$
{
    static class Program
    {
        const string
            Name = "$AssemblyName$",
            Signature = "$RootNamespace$",
            Description = "X-Plane plugin using FlyByWireless.XPLM";

        [UnmanagedCallersOnly(EntryPoint = "XPluginStart")]
        public static int Start(in byte name, in byte signature, in byte description)
        {
            static unsafe void S(in byte s, string value)
            {
                fixed (byte* p = &s)
                {
                    p[Encoding.UTF8.GetBytes(value, new(p, 255))] = 0;
                }
            }
            S(name, Name);
            S(signature, Signature);
            S(description, Description);
            Utilities.DebugString("Started.");
            return 1;
        }

        [UnmanagedCallersOnly(EntryPoint = "XPluginStop")]
        public static void Stop()
        {
            Utilities.DebugString("Stopped.");
        }

        [UnmanagedCallersOnly(EntryPoint = "XPluginEnable")]
        public static int Enable()
        {
            Utilities.DebugString("Enabled.");
            return 1;
        }

        [UnmanagedCallersOnly(EntryPoint = "XPluginDisable")]
        public static void Disable()
        {
            Utilities.DebugString("Disabled.");
        }

        [UnmanagedCallersOnly(EntryPoint = "XPluginReceiveMessage")]
        public static void ReceiveMessage(int from, int message, nint param)
        {
            // TODO: process message from another plugin
        }
    }
}