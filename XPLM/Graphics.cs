using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace FlyByWireless.XPLM;

public static class Graphics
{
    public static void SetState(int numberTexUnits, bool alphaTesting, bool alphaBlending, bool depthTesting, bool depthWriting)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMSetGraphicsState(int fog, int numberTexUnits, int lighting, int alphaTesting, int alphaBlending, int depthTesting, int depthWriting);

        XPLMSetGraphicsState(0, numberTexUnits, 0, alphaTesting ? 1 : 0, alphaBlending ? 1 : 0, depthTesting ? 1 : 0, depthWriting ? 1 : 0);
    }

    public static void BindTexture2d(int textureNum, int textureUnit)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMBindTexture2d(int textureNum, int textureUnit);

        XPLMBindTexture2d(textureNum, textureUnit);
    }

    public static void GenerateTextureNumbers(Span<int> textureIDs)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMGenerateTextureNumbers(ref int outTextureIDs, int count);

        XPLMGenerateTextureNumbers(ref MemoryMarshal.GetReference(textureIDs), textureIDs.Length);
    }

    public static void WorldToLocal(double latitude, double longitude, double altitude, out double x, out double y, out double z)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMWorldToLocal(double latitude, double longitude, double altitude, out double x, out double y, out double z);

        XPLMWorldToLocal(latitude, longitude, altitude, out x, out y, out z);
    }

    public static void LocalToWorld(double x, double y, double z, out double latitude, out double longitude, out double altitude)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMLocalToWorld(double x, double y, double z, out double latitude, out double longitude, out double altitude);

        XPLMLocalToWorld(x, y, z, out latitude, out longitude, out altitude);
    }

    public static void DrawTranslucentDarkBox(int left, int top, int right, int bottom)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMDrawTranslucentDarkBox(int left, int top, int right, int bottom);

        XPLMDrawTranslucentDarkBox(left, top, right, bottom);
    }
}

public static class Text
{
    public enum FontID
    {
        Basic,
        Proportional = 18
    }

    public static void DrawString(ReadOnlySpan<float> colorRGB, int xOffset, int yOffset, string charString, int? wordWrapWidth, FontID fontID)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMDrawString(in float colorRGB, int xOffset, int yOffset, [MarshalAs(UnmanagedType.LPUTF8Str)] string charString, in int wordWrapWidth, FontID fontID);

        var w = wordWrapWidth ?? default;
        XPLMDrawString(in MemoryMarshal.GetReference(colorRGB), xOffset, yOffset, charString, in wordWrapWidth.HasValue ? ref w : ref Unsafe.NullRef<int>(), fontID);
    }

    public static void DrawString(ReadOnlySpan<float> colorRGB, int xOffset, int yOffset, ReadOnlySpan<byte> charString, int? wordWrapWidth, FontID fontID)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMDrawString(in float colorRGB, int xOffset, int yOffset, in byte charString, in int wordWrapWidth, FontID fontID);

        var w = wordWrapWidth ?? default;
        XPLMDrawString(in MemoryMarshal.GetReference(colorRGB), xOffset, yOffset, in MemoryMarshal.GetReference(charString), in wordWrapWidth.HasValue ? ref w : ref Unsafe.NullRef<int>(), fontID);
    }

    public static void DrawNumber(ReadOnlySpan<float> colorRGB, int xOffset, int yOffset, double value, int digits, int decimals, bool showSign, FontID fontID)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMDrawNumber(in float colorRGB, int xOffset, int yOffset, double value, int digits, int decimals, int showSign, FontID fontID);

        XPLMDrawNumber(in MemoryMarshal.GetReference(colorRGB), xOffset, yOffset, value, digits, decimals, showSign ? 1 : 0, fontID);
    }

    public static void GetFontDimensions(FontID fontID, out int charWidth, out int charHeight, out bool digitsOnly)
    {
        [DllImport(Defs.Lib)]
        static extern void XPLMGetFontDimensions(FontID fontID, out int charWidth, out int charHeight, out int digitsOnly);

        XPLMGetFontDimensions(fontID, out charWidth, out charHeight, out var d);
        digitsOnly = d != 0;
    }

    public static float MeasureString(FontID fontID, string charString)
    {
        [DllImport(Defs.Lib)]
        static extern float XPLMMeasureString(FontID fontID, [MarshalAs(UnmanagedType.LPUTF8Str)] string charString, int numChars);

        return XPLMMeasureString(fontID, charString, charString.Length);
    }

    public static float MeasureString(FontID fontID, ReadOnlySpan<byte> charString)
    {
        [DllImport(Defs.Lib)]
        static extern float XPLMMeasureString(FontID fontID, in byte charString, int numChars);

        return XPLMMeasureString(fontID, in MemoryMarshal.GetReference(charString), charString.Length);
    }
}