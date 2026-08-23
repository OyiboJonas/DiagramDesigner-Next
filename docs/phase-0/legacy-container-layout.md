# Legacy compatibility source notes

The first real DDD fixtures confirm that the legacy Pascal source must be treated literally at the binary boundary.

For `TDiagramContainer`, the decoded order is:

1. `SaveString16(DefaultFontName)`
2. `DefaultFontSize` (`i32`)
3. `DefaultFontStyle` (`i32`)
4. `DefaultFontCharSet` (`u8`, file version >= 23; otherwise default 1)
5. `ObjectShadows` (`u8 bool`, file version >= 19; otherwise false)
6. `AutoLineBreak` (`u8 bool`, file version >= 21; otherwise false)
7. `ConnectorLabelStyle` (`u8`, file version >= 27; otherwise `clsSolid` / 1)
8. page count (`u16`)
9. page records
10. stencil layer (file version >= 5)

A page begins with width (`i32`), height (`i32`), `SaveString16` name and layer count (`u16`). A layer begins with `DrawColor` (`i32`) followed by the inherited object list: object count (`u16`), then one-byte object type plus each type-specific payload.

The parser deliberately stops at the first object type until payload codecs are implemented. It must never guess an object payload size merely to reach later records.
