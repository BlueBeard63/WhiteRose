#include "whiterose-calculator/cpp/clipboard.h"
#include <QClipboard>
#include <QGuiApplication>

void calculatorCopy(rust::Str text)
{
    QGuiApplication::clipboard()->setText(QString::fromUtf8(text.data(), qsizetype(text.size())));
}
