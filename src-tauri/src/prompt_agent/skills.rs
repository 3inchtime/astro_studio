use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PromptSkill {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub system_hint: &'static str,
}

pub fn built_in_skills() -> Vec<PromptSkill> {
    vec![
        PromptSkill {
            id: "gpt_image_2_director",
            name: "GPT Image 2 Template Director",
            description: "Adapts the Garden GPT Image 2 template workflow to Astro Studio's deep-thinking prompt flow.",
            system_hint: "Use the Garden GPT Image 2 methodology as prompt-engineering guidance only. Choose the closest template category, map user facts into structured fields, ask one precise question only when a missing field materially changes the result, then produce a draft prompt and parameter suggestions. Do not call image generation, do not mention Garden scripts, environment variables, prompt files, or local output folders. In Astro Studio, the user must accept the draft prompt before the existing generation flow runs.",
        },
        PromptSkill {
            id: "gpt_image_2_ui_mockups",
            name: "GPT Image 2 UI Mockups",
            description: "Structured templates for UI screenshots, app surfaces, social posts, live commerce, chat scenes, covers, and landing pages.",
            system_hint: "For UI mockups, structure the prompt around platform/context, screen type, visible content, layout regions, hierarchy, components, realistic UI states, device/frame, and brand palette. Use templates such as live commerce UI, social interface mockup, product card overlay, chat interface scene, short-video cover UI, and landing-page case study. Ask for exact UI text only when text is central; otherwise use short plausible labels and avoid long exact copy.",
        },
        PromptSkill {
            id: "gpt_image_2_product_visuals",
            name: "GPT Image 2 Product Visuals",
            description: "Templates for e-commerce product images, premium studio shots, packaging showcases, lifestyle scenes, and exploded views.",
            system_hint: "For product visuals, keep the product inspectable. Specify product geometry, material, finish, logo/label visibility, surface, lighting, shadows, reflections, camera angle, supporting props, callouts, and composition. Use templates such as white-background product, premium studio product, packaging showcase, lifestyle product scene, exploded-view poster, and e-commerce marketing board.",
        },
        PromptSkill {
            id: "gpt_image_2_maps_infographics",
            name: "GPT Image 2 Maps & Infographics",
            description: "Templates for illustrated maps, route maps, high-density explainers, comparison graphics, dashboards, and step-by-step infographics.",
            system_hint: "For maps and infographics, define title, subject, sections, legend, numbered locations or steps, visual encoding, density, hierarchy, and constraints. Use templates such as food map, travel route map, illustrated city map, store distribution map, itinerary day trip map, legend-heavy infographic, hand-drawn infographic, bento grid, comparison infographic, step-by-step infographic, and KPI dashboard. Do not invent quantitative data, coordinates, or rankings unless the user supplies them or asks for fictional data.",
        },
        PromptSkill {
            id: "gpt_image_2_slides_docs",
            name: "GPT Image 2 Slides & Visual Docs",
            description: "Templates for one-page explainers, policy slides, visual report pages, and educational diagram slides.",
            system_hint: "For slides and visual docs, specify page type, audience, headline system, information density, visual blocks, annotation style, and reading order. Use templates such as dense explainer slide, policy-style slide, visual report page, and educational diagram slide. Keep text short, scannable, and layout-safe.",
        },
        PromptSkill {
            id: "gpt_image_2_posters_campaigns",
            name: "GPT Image 2 Posters & Campaigns",
            description: "Templates for brand posters, campaign key visuals, web heroes, editorial covers, lineup posters, and concept posters.",
            system_hint: "For posters and campaigns, define the offer or idea, hero subject, composition, headline intent, typography style, palette, texture, campaign system, variants, and negative constraints. Use templates such as brand poster, campaign KV, banner hero, editorial cover, biomimetic concept poster, vintage editorial infographic, character catalog poster, and lineup comparison poster. Avoid asking the image model to render long exact text.",
        },
        PromptSkill {
            id: "gpt_image_2_portraits_characters",
            name: "GPT Image 2 Portraits & Characters",
            description: "Templates for professional portraits, founder portraits, virtual hosts, character sheets, and pose reference sheets.",
            system_hint: "For portraits and characters, clarify identity-safe subject description, silhouette, pose, expression, wardrobe, materials, lighting, background, consistency requirements, and output format. Use templates such as professional portrait, founder portrait, virtual host, character sheet, and pose reference sheet. Do not imply a real person's identity unless explicitly supplied by the user.",
        },
        PromptSkill {
            id: "gpt_image_2_scenes_illustrations",
            name: "GPT Image 2 Scenes & Illustrations",
            description: "Templates for healing scenes, cinematic concept art, picture-book scenes, and minimalist mood images.",
            system_hint: "For scenes and illustrations, define subject, setting, era, atmosphere, season/weather, composition, color script, lighting, storytelling cues, and medium. Use templates such as healing scene, concept scene, picture-book scene, and minimalist mood scene. Preserve the user's core idea and add only imageable detail.",
        },
        PromptSkill {
            id: "gpt_image_2_editing_workflows",
            name: "GPT Image 2 Editing Workflows",
            description: "Templates for image-edit prompts including background replacement, local object replacement, removal, retouching, and portrait edits.",
            system_hint: "For edit workflows, explicitly separate what must change from what must remain unchanged. Mention source-image consistency, identity/product preservation, lighting match, perspective, shadows, edges, and artifacts to avoid. Use templates such as background replacement, local object replacement, object removal, product retouching, and portrait local edit. The agent still only returns a draft prompt; Astro Studio performs the edit after user acceptance.",
        },
        PromptSkill {
            id: "gpt_image_2_avatars_profiles",
            name: "GPT Image 2 Avatars & Profiles",
            description: "Templates for style-transfer selfies, character portrait grids, themed 3D icons, stickers, and cultural portrait series.",
            system_hint: "For avatars and profiles, define persona, reference consistency, style, expression set, grid or sticker count, background, outline, lighting, and usage context. Use templates such as style-transfer selfie, character-grid portrait, themed 3D icon, sticker set, and cultural portrait series.",
        },
        PromptSkill {
            id: "gpt_image_2_storyboards_sequences",
            name: "GPT Image 2 Storyboards & Sequences",
            description: "Templates for comics, manga pages, anime key visuals, relationship diagrams, recipe flows, TVC storyboards, and cinematic grids.",
            system_hint: "For storyboards and sequences, define panel count, sequence order, scene continuity, camera direction, captions/dialogue intent, character consistency, and reading direction. Use templates such as four-panel comic, manga spread, anime key visual, character relationship diagram, recipe process flowchart, product TVC storyboard, cinematic storyboard grid, and process photo board.",
        },
        PromptSkill {
            id: "gpt_image_2_grids_collages",
            name: "GPT Image 2 Grids & Collages",
            description: "Templates for multi-panel banner sets, lookbook grids, mixed-style panels, pitch boards, and ad-banner grids.",
            system_hint: "For grids and collages, specify grid dimensions, per-panel subject, shared style system, variation rules, spacing, labels, and consistency constraints. Use templates such as 2x2 banner grid, lookbook grid, mixed-style multi-panel, anime pitch board, and ad-banner multi-grid.",
        },
        PromptSkill {
            id: "gpt_image_2_branding_packaging",
            name: "GPT Image 2 Branding & Packaging",
            description: "Templates for brand identity boards, mascot kits, cosmetics packaging, beverage labels, and character merch boards.",
            system_hint: "For branding and packaging, define brand personality, logo intent, palette, typography direction, packaging form, material, label hierarchy, mascot/character use, applications, and mockups. Use templates such as brand identity board, mascot brand kit, cosmetic packaging, beverage label design, full mascot brand doc, and character merch board.",
        },
        PromptSkill {
            id: "gpt_image_2_typography_layout",
            name: "GPT Image 2 Typography & Text Layout",
            description: "Templates for title-safe posters and bilingual text-led visuals.",
            system_hint: "For typography-led images, define exact short title only when required, language pairing, type style, alignment, spacing, hierarchy, background texture, and safe zones. Use templates such as title-safe poster and bilingual layout visual. Keep text minimal because image models are unreliable with long exact text.",
        },
        PromptSkill {
            id: "gpt_image_2_assets_props",
            name: "GPT Image 2 Assets & Props",
            description: "Templates for icon sets, game screenshots, props, and reusable visual assets.",
            system_hint: "For asset and prop tasks, define asset count, style rules, orthographic or perspective view, background transparency/cleanliness, labels, consistency across items, and export intent. Use templates such as retro skeuomorphic icons and game screenshot mockup.",
        },
        PromptSkill {
            id: "gpt_image_2_academic_figures",
            name: "GPT Image 2 Academic Figures",
            description: "Templates for publication-ready research figures, graphical abstracts, pipelines, mechanisms, architectures, comparisons, and charts.",
            system_hint: "For academic figures, prefer white or publication-style backgrounds, precise geometry, limited sober palette, readable labels, and clear panel structure. Use templates such as method pipeline overview, neural network architecture, qualitative comparison grid, scientific schematic, mechanism diagram, multi-condition comparison, publication chart, graphical abstract, and research overview poster. Strictly do not fabricate quantitative data, formulas, color scales, or results.",
        },
        PromptSkill {
            id: "gpt_image_2_technical_diagrams",
            name: "GPT Image 2 Technical Diagrams",
            description: "Templates for architecture diagrams, flowcharts, sequence diagrams, state machines, ER diagrams, mind maps, and network topology images.",
            system_hint: "For technical diagrams, define diagram type, nodes, edges, groups/zones, labels, direction, legend, and visual encoding. Use templates such as system architecture, flowchart decision, sequence diagram, state machine, ER diagram, technical mind map, and network topology. Make clear this is a PNG-style visual diagram, not editable Mermaid/SVG.",
        },
        PromptSkill {
            id: "photography",
            name: "Photography Director",
            description: "Shapes camera, lens, light, mood, and realistic photographic detail.",
            system_hint: "Use camera position, lens feel, depth of field, lighting direction, exposure, color temperature, and realistic material detail. Avoid generic quality tags.",
        },
        PromptSkill {
            id: "composition",
            name: "Composition Architect",
            description: "Organizes subject placement, framing, negative space, and visual hierarchy.",
            system_hint: "Specify subject placement, frame shape, foreground/midground/background, scale relationships, visual focus, and how the eye should travel through the image.",
        },
        PromptSkill {
            id: "character_design",
            name: "Character Designer",
            description: "Develops character silhouette, costume, pose, expression, and identity-safe details.",
            system_hint: "Clarify silhouette, pose, expression, wardrobe, materials, era, personality cues, and what must remain consistent. Do not invent protected identities or real-person claims.",
        },
        PromptSkill {
            id: "product_shot",
            name: "Product Shot Stylist",
            description: "Builds commercial product imagery with surface, lighting, packaging, and clean composition.",
            system_hint: "Describe product geometry, material, finish, surface, reflections, label visibility, supporting props, background, and lighting setup. Keep the subject inspectable.",
        },
        PromptSkill {
            id: "poster_design",
            name: "Poster Art Director",
            description: "Creates editorial poster concepts with strong focal hierarchy and graphic style.",
            system_hint: "Use poster composition, typography intent if requested, graphic hierarchy, palette, contrast, texture, and print style. Avoid asking the image model to render long exact text.",
        },
        PromptSkill {
            id: "worldbuilding",
            name: "Worldbuilding Designer",
            description: "Expands settings, environments, architecture, culture, and atmosphere.",
            system_hint: "Clarify location, era, architecture, environment, weather, cultural objects, scale, atmosphere, and sensory detail while preserving the user's core idea.",
        },
        PromptSkill {
            id: "anime_style",
            name: "Anime Visual Designer",
            description: "Shapes anime and illustration prompts with clean style, expressive pose, and readable scene design.",
            system_hint: "Use anime illustration language, character readability, line quality, shading style, color palette, pose, expression, and background detail. Avoid overloaded style keywords.",
        },
        PromptSkill {
            id: "prompt_editor",
            name: "Prompt Editor",
            description: "Condenses the final result into a natural, production-ready image prompt.",
            system_hint: "Rewrite into concise, production-ready imageable language. Preserve user intent, constraints, named entities, and negative constraints. Output a prompt that can be used directly.",
        },
    ]
}

pub fn skill_by_id(id: &str) -> Option<PromptSkill> {
    built_in_skills().into_iter().find(|skill| skill.id == id)
}

pub fn skill_hints(skill_ids: &[String]) -> Vec<PromptSkill> {
    skill_ids.iter().filter_map(|id| skill_by_id(id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn built_in_skill_ids_are_unique() {
        let skills = built_in_skills();
        let ids = skills.iter().map(|skill| skill.id).collect::<HashSet<_>>();
        assert_eq!(ids.len(), skills.len());
    }

    #[test]
    fn prompt_editor_skill_is_available() {
        let skill = skill_by_id("prompt_editor").expect("prompt_editor skill should exist");
        assert!(skill.system_hint.contains("production-ready"));
    }

    #[test]
    fn gpt_image_2_director_preserves_astro_generation_boundary() {
        let skill = skill_by_id("gpt_image_2_director")
            .expect("gpt_image_2_director skill should exist");
        assert!(skill.system_hint.contains("Do not call image generation"));
        assert!(skill.system_hint.contains("draft prompt"));
    }

    #[test]
    fn gpt_image_2_template_categories_are_available() {
        for id in [
            "gpt_image_2_ui_mockups",
            "gpt_image_2_product_visuals",
            "gpt_image_2_maps_infographics",
            "gpt_image_2_slides_docs",
            "gpt_image_2_posters_campaigns",
            "gpt_image_2_portraits_characters",
            "gpt_image_2_scenes_illustrations",
            "gpt_image_2_editing_workflows",
            "gpt_image_2_avatars_profiles",
            "gpt_image_2_storyboards_sequences",
            "gpt_image_2_grids_collages",
            "gpt_image_2_branding_packaging",
            "gpt_image_2_typography_layout",
            "gpt_image_2_assets_props",
            "gpt_image_2_academic_figures",
            "gpt_image_2_technical_diagrams",
        ] {
            assert!(skill_by_id(id).is_some(), "{id} should be registered");
        }
    }
}
