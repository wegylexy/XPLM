using System;
using System.Text;

namespace FlyByWireless.XPLM
{
    [Flags]
    public enum KeyFlags
    {
        Shift = 1,
        OptionAlt = 2,
        Control = 4,
        Down = 8,
        Up = 16
    }

    public enum ASCIIControlKey : byte
    {
        Return = 13,
        Escape = 27,
        Tab = 9,
        Delete = 8,
        Left = 28,
        Right = 29,
        Up = 30,
        Down = 31,
        Num0 = 48,
        Num1,
        Num2,
        Num3,
        Num4,
        Num5,
        Num6,
        Num7,
        Num8,
        Num9,
        Decimal = 46
    }

    public enum VirtualKey : byte
    {
        Back = 0x08,
        Tab = 0x09,
        Clear = 0x0C,
        Return = 0x0D,
        Escape = 0x1B,
        Space = 0x20,
        Prior,
        Next,
        End,
        Home,
        Left,
        Up,
        Right,
        Down,
        Select,
        Print,
        Execute,
        Snapshot,
        Insert,
        Delete,
        Help,
        Num0,
        Num1,
        Num2,
        Num3,
        Num4,
        Num5,
        Num6,
        Num7,
        Num8,
        Num9,
        A = 0x41,
        B,
        C,
        D,
        E,
        F,
        G,
        H,
        I,
        J,
        K,
        L,
        M,
        N,
        O,
        P,
        Q,
        R,
        S,
        T,
        U,
        V,
        W,
        X,
        Y,
        Z,
        NumPad0 = 0x60,
        NumPad1,
        NumPad2,
        NumPad3,
        NumPad4,
        NumPad5,
        NumPad6,
        NumPad7,
        NumPad8,
        NumPad9,
        Multiply,
        Add,
        Separator,
        Subtract,
        Decimal,
        Divide,
        F1,
        F2,
        F3,
        F4,
        F5,
        F6,
        F7,
        F8,
        F9,
        F10,
        F11,
        F12,
        F13,
        F14,
        F15,
        F16,
        F17,
        F18,
        F19,
        F20,
        F21,
        F22,
        F23,
        F24,
        Equal = 0xB0,
        Minux,
        RBrace,
        LBrace,
        Quote,
        Semicolon,
        Backslash,
        Comma,
        Slash,
        Period,
        Backquote,
        Enter,
        NumPadEnter,
        NumPadEqual
    }

    internal static class Defs
    {
        internal const string Lib =
#if IBM
            "XPLM_64"
#else
            "XPLM"
#endif
            ;

        internal static string GetString(this ref Span<byte> utf8)
        {
            var length = 0;
            while (length < utf8.Length && utf8[length] != 0)
                ++length;
            return Encoding.UTF8.GetString(utf8[..length]);
        }
    }
}