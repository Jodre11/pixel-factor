<#import "template.ftl" as layout>
<@layout.registrationLayout displayMessage=true; section>
<!-- template: pixel-factor-tap.ftl -->
    <#if section="header">
        <#if pfMode == "login">${msg("pixelFactorLoginTitle")}
        <#elseif pfMode == "enrol">${msg("pixelFactorEnrolTitle")}
        <#else>${msg("pixelFactorConfirmTitle")}
        </#if>
    <#elseif section="form">
        <p id="pf-instructions">
            <#if pfMode == "login">${msg("pixelFactorLoginHelp")}
            <#elseif pfMode == "enrol">${msg("pixelFactorEnrolHelp")}
            <#else>${msg("pixelFactorConfirmHelp")}
            </#if>
        </p>
        <#if pfWarnings?has_content>
            <ul id="pf-warnings">
                <#list pfWarnings as warning>
                    <li>${warning}</li>
                </#list>
            </ul>
        </#if>
        <form id="pf-form" class="${properties.kcFormClass!}" action="${url.loginAction}" method="post">
            <div id="pf-stage" data-max-taps="${pfMaxTaps?c}" data-filler="${pfFiller}">
                <img id="pf-image" src="${url.resourcesPath}/img/${pfImageFile}" alt="${msg("pixelFactorImageAlt")}"
                     draggable="false">
            </div>
            <input type="hidden" id="pf-taps" name="taps" value="">
            <div class="${properties.kcFormGroupClass!}" id="pf-controls">
                <button type="button" id="pf-undo" class="${properties.kcButtonClass!} ${properties.kcButtonDefaultClass!}">${msg("pixelFactorUndo")}</button>
                <button type="button" id="pf-clear" class="${properties.kcButtonClass!} ${properties.kcButtonDefaultClass!}">${msg("pixelFactorClear")}</button>
                <#if pfMode == "confirm">
                    <button type="submit" name="restart" value="1" formnovalidate class="${properties.kcButtonClass!} ${properties.kcButtonDefaultClass!}">${msg("pixelFactorRestart")}</button>
                </#if>
                <button type="submit" id="pf-submit" class="${properties.kcButtonClass!} ${properties.kcButtonPrimaryClass!}">${msg("doSubmit")}</button>
            </div>
        </form>
        <link rel="stylesheet" href="${url.resourcesPath}/css/pixel-factor.css">
        <script src="${url.resourcesPath}/js/pixel-factor-tap.js"></script>
    </#if>
</@layout.registrationLayout>
