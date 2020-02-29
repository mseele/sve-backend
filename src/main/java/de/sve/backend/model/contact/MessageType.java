package de.sve.backend.model.contact;

import de.sve.backend.mail.MailAccount;

public enum MessageType {
	General("Allgemein", MailAccount.INFO), //$NON-NLS-1$
	Events("Events", MailAccount.EVENTS), //$NON-NLS-1$
	Fitness("Fitness", MailAccount.FITNESS), //$NON-NLS-1$
	Kunstrasen("Kunstrasen", MailAccount.KUNSTRASEN); //$NON-NLS-1$

	private String fDisplayName;

	private MailAccount fMailAccount;

	private MessageType(String displayName, MailAccount mailAccount) {
		this.fDisplayName = displayName;
		this.fMailAccount = mailAccount;
	}

	public String displayName() {
		return this.fDisplayName;
	}
	
	public MailAccount mailAccount() {
		return this.fMailAccount;
	}

}
