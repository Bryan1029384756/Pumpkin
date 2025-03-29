use crate::entity::player::Player;
use pumpkin_protocol::client::play::{BosseventAction, CBossEvent};
use pumpkin_util::text::TextComponent;
use uuid::Uuid;

#[derive(Clone, PartialEq)]
pub enum BossbarColor {
    Pink,
    Blue,
    Red,
    Green,
    Yellow,
    Purple,
    White,
}

#[derive(Clone, PartialEq)]
pub enum BossbarDivisions {
    NoDivision,
    Notches6,
    Notches10,
    Notches12,
    Notches20,
}

#[derive(Clone)]
pub enum BossbarFlags {
    NoFlags,
    DarkenSky = 0x01,
    DragonBar = 0x02,
    CreateFog = 0x04,
}

#[derive(Clone)]
pub struct Bossbar {
    pub uuid: Uuid,
    pub title: TextComponent,
    pub health: f32,
    pub color: BossbarColor,
    pub division: BossbarDivisions,
    pub flags: BossbarFlags,
}

impl Bossbar {
    #[must_use]
    pub fn new(title: TextComponent) -> Self {
        let uuid = Uuid::new_v4();

        Self {
            uuid,
            title,
            health: 0.0,
            color: BossbarColor::White,
            division: BossbarDivisions::NoDivision,
            flags: BossbarFlags::NoFlags,
        }
    }

    #[must_use]
    pub fn with_options(title: TextComponent, options: BossbarOptions) -> Self {
        let uuid = Uuid::new_v4();

        Self {
            uuid,
            title,
            health: options.health.unwrap_or(0.0),
            color: options.color.unwrap_or(BossbarColor::White),
            division: options.division.unwrap_or(BossbarDivisions::NoDivision),
            flags: options.flags.unwrap_or(BossbarFlags::NoFlags),
        }
    }

    #[must_use]
    pub fn builder(title: TextComponent) -> BossbarBuilder {
        BossbarBuilder::new(title)
    }
}
#[derive(Default)]
pub struct BossbarOptions {
    pub health: Option<f32>,
    pub color: Option<BossbarColor>,
    pub division: Option<BossbarDivisions>,
    pub flags: Option<BossbarFlags>,
}
pub struct BossbarBuilder {
    title: TextComponent,
    options: BossbarOptions,
}

impl BossbarBuilder {
    #[must_use]
    pub fn new(title: TextComponent) -> Self {
        Self {
            title,
            options: BossbarOptions::default(),
        }
    }

    #[must_use]
    pub fn health(mut self, health: f32) -> Self {
        self.options.health = Some(health);
        self
    }

    #[must_use]
    pub fn color(mut self, color: BossbarColor) -> Self {
        self.options.color = Some(color);
        self
    }

    #[must_use]
    pub fn division(mut self, division: BossbarDivisions) -> Self {
        self.options.division = Some(division);
        self
    }

    #[must_use]
    pub fn flags(mut self, flags: BossbarFlags) -> Self {
        self.options.flags = Some(flags);
        self
    }

    #[must_use]
    pub fn build(self) -> Bossbar {
        Bossbar::with_options(self.title, self.options)
    }
}

/// Extra methods for [`Player`] to send and manage the bossbar.
impl Player {
    pub async fn send_bossbar(&self, bossbar: &Bossbar) {
        // Maybe this section could be implemented. Feel free to change it.
        let bossbar = bossbar.clone();
        let boss_action = BosseventAction::Add {
            title: bossbar.title,
            health: bossbar.health,
            color: (bossbar.color as u8).into(),
            division: (bossbar.division as u8).into(),
            flags: bossbar.flags as u8,
        };

        let packet = CBossEvent::new(&bossbar.uuid, boss_action);
        self.client.enqueue_packet(&packet).await;
    }
    pub async fn remove_bossbar(&self, uuid: Uuid) {
        let boss_action = BosseventAction::Remove;

        let packet = CBossEvent::new(&uuid, boss_action);
        self.client.enqueue_packet(&packet).await;
    }

    pub async fn update_bossbar_health(&self, uuid: &Uuid, health: f32) {
        let boss_action = BosseventAction::UpdateHealth(health);

        let packet = CBossEvent::new(uuid, boss_action);
        self.client.enqueue_packet(&packet).await;
    }

    pub async fn update_bossbar_title(&self, uuid: &Uuid, title: TextComponent) {
        let boss_action = BosseventAction::UpdateTile(title);

        let packet = CBossEvent::new(uuid, boss_action);
        self.client.enqueue_packet(&packet).await;
    }

    pub async fn update_bossbar_style(
        &self,
        uuid: &Uuid,
        color: BossbarColor,
        dividers: BossbarDivisions,
    ) {
        let boss_action = BosseventAction::UpdateStyle {
            color: (color as u8).into(),
            dividers: (dividers as u8).into(),
        };

        let packet = CBossEvent::new(uuid, boss_action);
        self.client.enqueue_packet(&packet).await;
    }

    pub async fn update_bossbar_flags(&self, uuid: &Uuid, flags: BossbarFlags) {
        let boss_action = BosseventAction::UpdateFlags(flags as u8);

        let packet = CBossEvent::new(uuid, boss_action);
        self.client.enqueue_packet(&packet).await;
    }
}
