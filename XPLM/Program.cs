using System.Runtime.InteropServices;
using System.Text;

namespace FlyByWireless.XPLM
{
    static class Program
    {
        const string
            Name = "Fly by Wireless",
            Signature = "hk.timtim.flybywireless",
            Description = "P/Invoke XPLM plugin template.";

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
            return 1;
        }

        [UnmanagedCallersOnly(EntryPoint = "XPluginStop")]
        public static void Stop() { }

        [UnmanagedCallersOnly(EntryPoint = "XPluginEnable")]
        public static int Enable()
        {
            // TODO: enable the plugin
            return 1;
        }

        [UnmanagedCallersOnly(EntryPoint = "XPluginDisable")]
        public static void Disable()
        {
            // TODO: disable the plugin
        }

        [UnmanagedCallersOnly(EntryPoint = "XPluginReceiveMessage")]
        public static void ReceiveMessage(int from, int message, nint param)
        {
            // TODO: process message from another plugin
        }
    }
}