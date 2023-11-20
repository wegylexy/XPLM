using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace FlyByWireless.XPLM;

public static partial class Graphics
{
    [LibraryImport(Defs.Lib)]
    private static partial void XPLMSetGraphicsState(int fog, int numberTexUnits, int lighting, int alphaTesting, int alphaBlending, int depthTesting, int depthWriting);

    public static void SetState(int numberTexUnits, bool alphaTesting, bool alphaBlending, bool depthTesting, bool depthWriting)
    {
        XPLMSetGraphicsState(0, numberTexUnits, 0, alphaTesting ? 1 : 0, alphaBlending ? 1 : 0, depthTesting ? 1 : 0, depthWriting ? 1 : 0);
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMBindTexture2d(int textureNum, int textureUnit);

    public static void BindTexture2d(int textureNum, int textureUnit)
    {
        XPLMBindTexture2d(textureNum, textureUnit);
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMGenerateTextureNumbers(ref int outTextureIDs, int count);

    public static void GenerateTextureNumbers(Span<int> textureIDs)
    {
        XPLMGenerateTextureNumbers(ref MemoryMarshal.GetReference(textureIDs), textureIDs.Length);
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMWorldToLocal(double latitude, double longitude, double altitude, out double x, out double y, out double z);

    public static void WorldToLocal(double latitude, double longitude, double altitude, out double x, out double y, out double z)
    {
        XPLMWorldToLocal(latitude, longitude, altitude, out x, out y, out z);
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMLocalToWorld(double x, double y, double z, out double latitude, out double longitude, out double altitude);

    public static void LocalToWorld(double x, double y, double z, out double latitude, out double longitude, out double altitude)
    {
        XPLMLocalToWorld(x, y, z, out latitude, out longitude, out altitude);
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMDrawTranslucentDarkBox(int left, int top, int right, int bottom);

    public static void DrawTranslucentDarkBox(int left, int top, int right, int bottom)
    {
        XPLMDrawTranslucentDarkBox(left, top, right, bottom);
    }
}

public static partial class Text
{
    public enum FontID
    {
        Basic,
        Proportional = 18
    }

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static partial void XPLMDrawString(in float colorRGB, int xOffset, int yOffset, string charString, in int wordWrapWidth, FontID fontID);

    public static void DrawString(ReadOnlySpan<float> colorRGB, int xOffset, int yOffset, string charString, int? wordWrapWidth, FontID fontID)
    {
        var w = wordWrapWidth ?? default;
        XPLMDrawString(in MemoryMarshal.GetReference(colorRGB), xOffset, yOffset, charString, in wordWrapWidth.HasValue ? ref w : ref Unsafe.NullRef<int>(), fontID);
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMDrawString(in float colorRGB, int xOffset, int yOffset, in byte charString, in int wordWrapWidth, FontID fontID);

    public static void DrawString(ReadOnlySpan<float> colorRGB, int xOffset, int yOffset, ReadOnlySpan<byte> charString, int? wordWrapWidth, FontID fontID)
    {
        var w = wordWrapWidth ?? default;
        XPLMDrawString(in MemoryMarshal.GetReference(colorRGB), xOffset, yOffset, in MemoryMarshal.GetReference(charString), in wordWrapWidth.HasValue ? ref w : ref Unsafe.NullRef<int>(), fontID);
    }

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMDrawNumber(in float colorRGB, int xOffset, int yOffset, double value, int digits, int decimals, int showSign, FontID fontID);

    public static void DrawNumber(ReadOnlySpan<float> colorRGB, int xOffset, int yOffset, double value, int digits, int decimals, bool showSign, FontID fontID) =>
        XPLMDrawNumber(in MemoryMarshal.GetReference(colorRGB), xOffset, yOffset, value, digits, decimals, showSign ? 1 : 0, fontID);

    [LibraryImport(Defs.Lib)]
    private static partial void XPLMGetFontDimensions(FontID fontID, out int charWidth, out int charHeight, out int digitsOnly);

    public static void GetFontDimensions(FontID fontID, out int charWidth, out int charHeight, out bool digitsOnly)
    {
        XPLMGetFontDimensions(fontID, out charWidth, out charHeight, out var d);
        digitsOnly = d != 0;
    }

    [LibraryImport(Defs.Lib, StringMarshalling = StringMarshalling.Utf8)]
    private static partial float XPLMMeasureString(FontID fontID, string charString, int numChars);

    public static float MeasureString(FontID fontID, string charString) =>
        XPLMMeasureString(fontID, charString, charString.Length);

    [LibraryImport(Defs.Lib)]
    private static partial float XPLMMeasureString(FontID fontID, in byte charString, int numChars);

    public static float MeasureString(FontID fontID, ReadOnlySpan<byte> charString) =>
        XPLMMeasureString(fontID, in MemoryMarshal.GetReference(charString), charString.Length);
}