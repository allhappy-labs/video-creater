/*
 * Copyright (c) 2026 Video Creater contributors.
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

#include "tvgStr.h"
#include "tvgMath.h"
#include "tvgLottieInterpolator.h"

namespace
{
constexpr uint32_t SOLVER_ITERATIONS = 24;

float cubicCoordinate(float parameter, float firstControl, float secondControl)
{
    const auto inverse = 1.0f - parameter;
    return 3.0f * inverse * inverse * parameter * firstControl
         + 3.0f * inverse * parameter * parameter * secondControl
         + parameter * parameter * parameter;
}

float solveParameter(float progress, float firstControlX, float secondControlX)
{
    auto lower = 0.0f;
    auto upper = 1.0f;
    for (uint32_t iteration = 0; iteration < SOLVER_ITERATIONS; ++iteration) {
        const auto candidate = (lower + upper) * 0.5f;
        if (cubicCoordinate(candidate, firstControlX, secondControlX) < progress) lower = candidate;
        else upper = candidate;
    }
    return (lower + upper) * 0.5f;
}
}

float LottieInterpolator::progress(float value)
{
    if (value <= 0.0f) return 0.0f;
    if (value >= 1.0f) return 1.0f;
    if (outTangent.x == outTangent.y && inTangent.x == inTangent.y) return value;

    const auto parameter = solveParameter(value, outTangent.x, inTangent.x);
    return cubicCoordinate(parameter, outTangent.y, inTangent.y);
}

void LottieInterpolator::set(const char* valueKey, Point& incoming, Point& outgoing)
{
    key = duplicate(valueKey);
    inTangent = incoming;
    outTangent = outgoing;
}
