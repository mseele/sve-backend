package de.sve.backend.model.news;

import de.sve.backend.mail.MailAccount;

public enum NewsType {
	General("Allgemein", MailAccount.INFO), //$NON-NLS-1$
	Events("Events", MailAccount.EVENTS), //$NON-NLS-1$
	Fitness("Fitness", MailAccount.FITNESS); //$NON-NLS-1$

	private String fDisplayName;

	private MailAccount fMailAccount;

	private NewsType(String displayName, MailAccount mailAccount) {
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
