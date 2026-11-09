#include "whiterose-files/cpp/image.h"

#include <QImageIOHandler>
#include <QImageReader>

rust::String imageSize(rust::Str path)
{
    if (path.empty())
        return rust::String();
    QImageReader reader(QString::fromUtf8(path.data(), qsizetype(path.size())));
    QSize s = reader.size();
    if (!s.isValid())
        return rust::String();
    if (reader.transformation() & QImageIOHandler::TransformationRotate90)
        s.transpose();
    const QByteArray out = QByteArray::number(s.width()) + " × " + QByteArray::number(s.height());
    return rust::String(out.constData(), size_t(out.size()));
}
